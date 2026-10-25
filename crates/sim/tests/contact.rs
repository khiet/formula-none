//! How karts bounce off each other, at the sim's public seam: start two
//! karts on the oval's front straight, drive one into the other and read
//! the transforms the sim reports. The oval's front straight runs along
//! -Z at x = 30 with asphalt 6 m either side (trackgen/layouts/oval.py),
//! so a kart in slot 0's column at x = 28 and one at x = 31 drive side by
//! side.

use sim::tuning::{
    KART_BUMP_DECAY_S, KART_BUMP_SPEED_LOSS, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M,
    TICKS_PER_SECOND, TOP_SPEED_M_PER_S,
};
use sim::{step, KartInput, KartReadout, Phase, Prediction, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;
/// Centres this far apart along the column are nose to tail.
const NOSE_TO_TAIL_M: f32 = 2.0 * KART_HALF_LENGTH_M;
/// Centres this far apart across the column are side by side.
const SIDE_BY_SIDE_M: f32 = 2.0 * KART_HALF_WIDTH_M;

/// The race past its countdown, every kart idle on the grid until Go.
fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
    }
    state
}

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

fn input(throttle: f32, steer: f32) -> KartInput {
    KartInput {
        throttle,
        steer,
        ..KartInput::default()
    }
}

/// The oval with its first spawn slots moved to `slots`, each a position
/// and a yaw, so two karts can be started wherever the scenario needs.
fn oval_with_slots(slots: &[([f32; 3], f32)]) -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    for (slot, (position, yaw)) in slots.iter().enumerate() {
        sidecar["spawn_grid"][slot] = serde_json::json!({ "position": position, "yaw": yaw });
    }
    Track::from_sidecar(&sidecar.to_string()).expect("the oval with moved slots loads")
}

/// The two karts' transforms after each of `seconds` of the inputs
/// `drive` gives for the tick, from Go; `drive` also sees the transforms
/// after the last tick, as a player would, once there has been one.
fn drive_two(
    track: Track,
    seconds: f32,
    drive: impl Fn(usize, Option<&[KartReadout; 2]>) -> [KartInput; 2],
) -> Vec<[KartReadout; 2]> {
    let mut state = started(State::new(SEED, 2, track));
    let mut trace: Vec<[KartReadout; 2]> = Vec::new();
    for tick in 0..ticks(seconds) {
        state = step(state, &drive(tick, trace.last()), DT);
        let karts = state.kart_readouts();
        trace.push([karts[0], karts[1]]);
    }
    trace
}

/// How far apart the two karts' centres are along the ground.
fn apart(karts: &[KartReadout; 2]) -> f32 {
    let dx = karts[1].position[0] - karts[0].position[0];
    let dz = karts[1].position[2] - karts[0].position[2];
    (dx * dx + dz * dz).sqrt()
}

/// The first tick the two karts touch: their centres come within a tick
/// of travel of `touching`, nose to tail or side by side.
fn first_touch(trace: &[[KartReadout; 2]], touching: f32) -> usize {
    let closest = trace.iter().map(apart).fold(f32::MAX, f32::min);
    trace
        .iter()
        .position(|karts| apart(karts) < touching + TOUCH_MARGIN_M)
        .unwrap_or_else(|| panic!("the karts never touch: closest {closest:.2} m"))
}

/// A kart at top speed covers half a metre in a tick, so centres within
/// this of touching met this tick or the next.
const TOUCH_MARGIN_M: f32 = 0.5;

/// The first tick a kart driving with the throttle held loses speed to a
/// hit: more than a tick of acceleration could give back.
fn first_slowed(speeds: impl IntoIterator<Item = f32>) -> Option<usize> {
    let speeds: Vec<f32> = speeds.into_iter().collect();
    speeds
        .windows(2)
        .position(|pair| pair[1] < pair[0] - 1.0)
        .map(|index| index + 1)
}

/// The first tick the driver in slot 0 is slowed by a hit.
fn first_hit(trace: &[[KartReadout; 2]]) -> usize {
    first_slowed(trace.iter().map(|karts| karts[0].speed)).expect("the driver is slowed by a hit")
}

/// A hit square into a standing kart costs the driver the tuned share of
/// the closing speed, its own; this much of the speed may survive, with a
/// tenth to spare for the tick's acceleration and the solver's part.
const SURVIVING_SPEED_SHARE: f32 = 1.0 - KART_BUMP_SPEED_LOSS + 0.1;

/// Seconds for a bump to decay to nothing a kart could show: five of its
/// decay times leave under a percent of it.
const BUMP_SPENT_S: f32 = 5.0 * KART_BUMP_DECAY_S;

#[test]
fn a_kart_driven_into_the_back_of_another_bounces_off_and_slows_instead_of_pushing_it_along() {
    // Slot 0 at full throttle, 10 m behind an idle kart in its column.
    let track = oval_with_slots(&[([28.0, 0.0, 4.0], 0.0), ([28.0, 0.0, -6.0], 0.0)]);
    let trace = drive_two(track, 4.0, |_, _| [input(1.0, 0.0), KartInput::default()]);
    let touch = first_hit(&trace);
    assert!(
        apart(&trace[touch]) < NOSE_TO_TAIL_M + TOUCH_MARGIN_M,
        "the hit is the touch: {:.2} m apart",
        apart(&trace[touch])
    );
    let (before, after) = (trace[touch - 1][0], trace[touch][0]);
    assert!(before.speed > 10.0, "reached the idle kart at speed");
    assert!(
        after.speed < SURVIVING_SPEED_SHARE * before.speed,
        "the hit cost the driver the tuned share of the closing speed: {} then {}",
        before.speed,
        after.speed
    );
    let widest = trace[touch..touch + ticks(0.5)]
        .iter()
        .map(apart)
        .fold(0.0, f32::max);
    assert!(
        widest > NOSE_TO_TAIL_M + 0.3,
        "the karts bounced apart within half a second: {widest:.2} m at most"
    );
    let recoiled = trace[touch..touch + ticks(0.5)]
        .iter()
        .any(|karts| karts[0].velocity[2] > 0.5);
    assert!(recoiled, "the driver recoiled, moving back up the column");
    // The idle kart is nudged, never carried: it keeps its own speed of
    // nothing and never moves at the driver's pace.
    for karts in &trace[touch..] {
        let idle = karts[1];
        assert_eq!(idle.speed, 0.0, "the idle kart's speed is its own");
        assert!(
            idle.velocity[2].abs() < 0.5 * TOP_SPEED_M_PER_S,
            "the idle kart is not carried along: {:?}",
            idle.velocity
        );
    }
    let last = trace.last().expect("the trace has ticks");
    let shunted = trace[touch][1].position[2] - last[1].position[2];
    assert!(
        shunted < 10.0,
        "after {:.1} s of ramming the idle kart has only been nudged {shunted:.1} m",
        (trace.len() - touch) as f32 * DT
    );
}

#[test]
fn karts_that_touch_side_by_side_part_again_and_the_one_hit_keeps_its_speed() {
    // Both at full throttle in neighbouring columns; slot 0 steers into
    // slot 1 for half a second once both are up to speed, then straight.
    let track = oval_with_slots(&[([28.0, 0.0, 4.0], 0.0), ([31.0, 0.0, 4.0], 0.0)]);
    let trace = drive_two(track, 4.0, |tick, _| {
        let steer = if (ticks(1.5)..ticks(2.0)).contains(&tick) {
            0.4
        } else {
            0.0
        };
        [input(1.0, steer), input(1.0, 0.0)]
    });
    let touch = first_touch(&trace, SIDE_BY_SIDE_M);
    let parted = trace[touch..touch + ticks(0.5)]
        .iter()
        .any(|karts| apart(karts) > SIDE_BY_SIDE_M + 0.3);
    assert!(parted, "the karts bounced apart within half a second");
    let hit_before = trace[touch - 1][1].speed;
    for karts in &trace[touch..] {
        assert!(
            karts[1].speed >= hit_before,
            "a kart hit from beside loses no speed: {} then {}",
            hit_before,
            karts[1].speed
        );
    }
    // Neither kart is turned off down the straight by the touch.
    for karts in &trace[touch..] {
        for kart in karts {
            let nose = nose_direction(kart);
            assert!(nose[2] < -0.95, "still heading down the straight: {nose:?}");
        }
    }
}

#[test]
fn a_bump_dies_away_and_the_kart_hit_comes_to_rest_again() {
    // Slot 0 drives into the back of an idle kart, then brakes so it
    // cannot hit it again.
    let track = oval_with_slots(&[([28.0, 0.0, 4.0], 0.0), ([28.0, 0.0, -6.0], 0.0)]);
    let trace = drive_two(track, 4.0, |_, last| {
        let hit = last.is_some_and(|karts| karts[1].velocity[2] != 0.0);
        let driver = if hit {
            KartInput {
                brake: 1.0,
                ..KartInput::default()
            }
        } else {
            input(1.0, 0.0)
        };
        [driver, KartInput::default()]
    });
    let touch = first_hit(&trace);
    // The push shows in the velocity from the tick after the touch.
    let pushed = trace[touch..touch + 5]
        .iter()
        .map(|karts| -karts[1].velocity[2])
        .fold(0.0, f32::max);
    assert!(
        pushed > 5.0,
        "the kart hit is shunted on up the column: {pushed} m/s"
    );
    let settled = trace[touch + ticks(BUMP_SPENT_S)][1];
    let moving = (settled.velocity[0].powi(2) + settled.velocity[2].powi(2)).sqrt();
    assert!(
        moving < 0.1,
        "{BUMP_SPENT_S} s on the push is spent and the kart stands: {moving} m/s"
    );
}

#[test]
fn the_prediction_bounces_the_own_kart_off_a_kart_the_snapshots_draw() {
    // The own kart at full throttle; the snapshots draw another kart
    // standing 10 m up its column.
    let track = oval_with_slots(&[([28.0, 0.0, 4.0], 0.0), ([28.0, 0.0, -6.0], 0.0)]);
    let mut prediction = Prediction::new(SEED, 0, track.clone());
    let standing = started(State::new(SEED, 2, track)).kart_readouts();
    // Idle through the countdown, for a clean start, then full throttle.
    let countdown = ticks(sim::tuning::COUNTDOWN_S) as u32;
    let mut trace = Vec::new();
    for sequence in 1..=(countdown + ticks(3.0) as u32) {
        let input = if sequence <= countdown {
            KartInput::default()
        } else {
            input(1.0, 0.0)
        };
        prediction.tick(sequence, input, &standing);
        trace.push(prediction.kart_readout());
    }
    let gap = |kart: &KartReadout| kart.position[2] - standing[1].position[2];
    let touch =
        first_slowed(trace.iter().map(|kart| kart.speed)).expect("the own kart is slowed by a hit");
    assert!(
        gap(&trace[touch]) < NOSE_TO_TAIL_M + TOUCH_MARGIN_M,
        "the hit is the touch: {:.2} m apart",
        gap(&trace[touch])
    );
    assert!(
        trace[touch].speed < SURVIVING_SPEED_SHARE * trace[touch - 1].speed,
        "the hit cost the own kart the tuned share of the closing speed: {} then {}",
        trace[touch - 1].speed,
        trace[touch].speed
    );
    let recoiled = trace[touch..touch + ticks(0.5)]
        .iter()
        .any(|kart| kart.velocity[2] > 0.5);
    assert!(recoiled, "the own kart recoiled, moving back up the column");
    for kart in &trace[touch..] {
        assert!(
            kart.position[2] > standing[1].position[2] + NOSE_TO_TAIL_M - 0.2,
            "never through the standing kart: {:?}",
            kart.position
        );
    }
}

#[test]
fn a_correction_during_a_bump_keeps_the_own_karts_speed_and_recoil() {
    // The own kart at full throttle into the kart standing 10 m up its
    // column, as above; the tick after the touch a snapshot puts it a
    // hand's width aside, so the prediction rewinds to the snapshot.
    let track = oval_with_slots(&[([28.0, 0.0, 4.0], 0.0), ([28.0, 0.0, -6.0], 0.0)]);
    let mut prediction = Prediction::new(SEED, 0, track.clone());
    let standing = started(State::new(SEED, 2, track)).kart_readouts();
    let countdown = ticks(sim::tuning::COUNTDOWN_S) as u32;
    let drive = |sequence: u32| {
        if sequence <= countdown {
            KartInput::default()
        } else {
            input(1.0, 0.0)
        }
    };
    let mut sequence = 0;
    let mut speeds = Vec::new();
    while first_slowed(speeds.iter().copied()).is_none() {
        sequence += 1;
        prediction.tick(sequence, drive(sequence), &standing);
        speeds.push(prediction.kart_readout().speed);
        assert!(
            sequence < countdown + ticks(3.0) as u32,
            "the own kart is slowed by a hit"
        );
    }
    sequence += 1;
    prediction.tick(sequence, drive(sequence), &standing);
    let before = prediction.kart_readout();
    let mut aside = prediction.kart_readout();
    aside.position[0] += 0.3;

    let moved = prediction.reconcile(sequence, sequence, &aside);
    assert!(
        (moved - 0.3).abs() < 0.05,
        "rewound to the snapshot: moved {moved} m"
    );
    // A snapshot carries no bump, so the prediction keeps its own and
    // reads the speed from the snapshot's velocity with that bump taken
    // off, instead of counting the bump against the speed twice.
    let after = prediction.kart_readout();
    assert!(
        (after.speed - before.speed).abs() < 0.5,
        "the rewind keeps the speed the touch left: {} then {}",
        before.speed,
        after.speed
    );
    let mut recoiled = false;
    for _ in 0..ticks(0.5) {
        sequence += 1;
        prediction.tick(sequence, drive(sequence), &standing);
        let kart = prediction.kart_readout();
        recoiled |= kart.velocity[2] > 0.5;
        assert!(
            kart.position[2] > standing[1].position[2] + NOSE_TO_TAIL_M - 0.2,
            "never through the standing kart: {:?}",
            kart.position
        );
    }
    assert!(recoiled, "the recoil goes on after the correction");
}

/// The unit direction the kart's nose points along the ground.
fn nose_direction(kart: &KartReadout) -> [f32; 3] {
    let [x, y, z, w] = kart.rotation;
    // The quaternion's rotation of -Z.
    [
        -(2.0 * (x * z + w * y)),
        -(2.0 * (y * z - w * x)),
        -(1.0 - 2.0 * (x * x + y * y)),
    ]
}
