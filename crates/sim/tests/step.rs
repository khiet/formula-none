//! The sim's public seam: `step(state, inputs, dt)`. Tests drive a kart with
//! input sequences and assert the transforms the sim reports, never internals.
//! Races run on the oval fixture, whose front straight is clear for 100 m
//! past the start line and runs along -Z.

use sim::tuning::{
    ACCELERATION_TO_TOP_S, BRAKE_TO_STOP_S, COAST_TO_STOP_S, REVERSE_TOP_SPEED_M_PER_S,
    STEER_RATE_LOW_SPEED_RAD_PER_S, STEER_RATE_TOP_SPEED_RAD_PER_S, TICKS_PER_SECOND,
    TOP_SPEED_M_PER_S,
};
use sim::{step, KartInput, KartTransform, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;

fn race(kart_count: usize) -> State {
    State::new(
        SEED,
        kart_count,
        Track::from_sidecar(OVAL).expect("the oval fixture loads"),
    )
}

fn run(mut state: State, inputs: impl IntoIterator<Item = KartInput>) -> State {
    for input in inputs {
        state = step(state, &[input], DT);
    }
    state
}

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

fn only_kart(state: &State) -> KartTransform {
    let transforms = state.kart_transforms();
    assert_eq!(transforms.len(), 1);
    transforms[0]
}

/// Horizontal distance between two transforms.
fn distance_travelled(before: &KartTransform, after: &KartTransform) -> f32 {
    let dx = after.position[0] - before.position[0];
    let dz = after.position[2] - before.position[2];
    (dx * dx + dz * dz).sqrt()
}

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`.
fn yaw(transform: &KartTransform) -> f32 {
    let [x, y, z, w] = transform.rotation;
    (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z))
}

/// Signed speed along -Z between two transforms of a kart facing -Z: positive
/// is forward. Read from positions so the test never trusts the readout alone.
fn forward_speed(before: &KartTransform, after: &KartTransform) -> f32 {
    (before.position[2] - after.position[2]) / DT
}

/// Yaw rate measured over one more tick of `input`, in radians per second.
fn measured_yaw_rate(state: State, input: KartInput) -> f32 {
    let before = yaw(&only_kart(&state));
    let after = yaw(&only_kart(&step(state, &[input], DT)));
    (after - before) / DT
}

fn accelerate() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

fn brake() -> KartInput {
    KartInput {
        brake: 1.0,
        ..KartInput::default()
    }
}

fn drive_then_turn() -> Vec<KartInput> {
    let turn = KartInput {
        throttle: 1.0,
        steer: 1.0,
        ..KartInput::default()
    };
    let mut inputs = vec![accelerate(); ticks(3.0)];
    inputs.extend(vec![turn; ticks(2.0)]);
    inputs
}

#[test]
fn same_seed_and_inputs_give_the_same_final_transform() {
    let first = run(race(1), drive_then_turn());
    let second = run(race(1), drive_then_turn());

    let spawn = only_kart(&race(1));
    assert!(
        distance_travelled(&spawn, &only_kart(&first)) > 1.0,
        "the sequence must actually move the kart for the comparison to mean anything"
    );
    assert_eq!(only_kart(&first), only_kart(&second));
}

#[test]
fn a_kart_with_no_input_rests_on_the_track() {
    let spawn = only_kart(&race(1));
    let settled = run(race(1), vec![KartInput::default(); ticks(2.0)]);
    let after = only_kart(&settled);

    assert!(distance_travelled(&spawn, &after) < 0.01);
    assert!(
        (after.position[1] - spawn.position[1]).abs() < 0.05,
        "kart must not sink or float"
    );
}

#[test]
fn full_throttle_reaches_top_speed_in_the_tuned_time() {
    // One tick past the tuned ramp so the comparison is not on the boundary.
    let state = run(
        race(1),
        vec![accelerate(); ticks(ACCELERATION_TO_TOP_S) + 1],
    );
    let before = only_kart(&state);
    let state = step(state, &[accelerate()], DT);
    let after = only_kart(&state);

    let speed = distance_travelled(&before, &after) / DT;
    assert!(
        (speed - TOP_SPEED_M_PER_S).abs() < 0.5,
        "expected about {TOP_SPEED_M_PER_S} m/s, measured {speed}"
    );
}

#[test]
fn throttle_moves_the_kart_forward_along_its_facing() {
    let spawn = only_kart(&race(1));
    let after = only_kart(&run(race(1), vec![accelerate(); ticks(1.0)]));

    // Karts spawn facing -Z, the Three.js forward convention.
    assert!(after.position[2] < spawn.position[2] - 1.0);
    assert!((after.position[0] - spawn.position[0]).abs() < 0.01);
}

#[test]
fn steering_right_turns_the_kart_clockwise_seen_from_above() {
    let turn_right = KartInput {
        throttle: 1.0,
        steer: 1.0,
        ..KartInput::default()
    };
    let turn_left = KartInput {
        steer: -1.0,
        ..turn_right
    };
    let right = only_kart(&run(race(1), vec![turn_right; ticks(0.5)]));
    let left = only_kart(&run(race(1), vec![turn_left; ticks(0.5)]));

    // Clockwise from above is a negative rotation about +Y.
    assert!(yaw(&right) < -0.1);
    assert!(yaw(&left) > 0.1);
}

#[test]
fn top_speed_is_capped_however_long_the_throttle_is_held() {
    let state = run(
        race(1),
        vec![accelerate(); ticks(3.0 * ACCELERATION_TO_TOP_S)],
    );
    let before = only_kart(&state);
    let after = only_kart(&step(state, &[accelerate()], DT));

    assert!(forward_speed(&before, &after) <= TOP_SPEED_M_PER_S + 0.01);
    assert_eq!(after.speed, TOP_SPEED_M_PER_S);
}

#[test]
fn steer_rate_is_the_tuned_low_speed_value_when_barely_rolling() {
    // A few ticks of throttle: rolling, but a small fraction of top speed.
    let rolling = run(race(1), vec![accelerate(); 8]);
    let speed = only_kart(&rolling).speed;
    assert!(
        speed > 1.0 && speed < 0.1 * TOP_SPEED_M_PER_S,
        "{speed} m/s"
    );
    let turn = KartInput {
        steer: 1.0,
        ..KartInput::default()
    };

    let rate = measured_yaw_rate(rolling, turn).abs();
    assert!(
        (rate - STEER_RATE_LOW_SPEED_RAD_PER_S).abs() < 0.1,
        "expected about {STEER_RATE_LOW_SPEED_RAD_PER_S} rad/s, measured {rate}"
    );
}

#[test]
fn steer_rate_falls_to_the_tuned_top_speed_value() {
    let at_top_speed = run(
        race(1),
        vec![accelerate(); ticks(ACCELERATION_TO_TOP_S) + 1],
    );
    let turn = KartInput {
        steer: 1.0,
        ..accelerate()
    };

    let rate = measured_yaw_rate(at_top_speed, turn).abs();
    assert!(
        (rate - STEER_RATE_TOP_SPEED_RAD_PER_S).abs() < 0.02,
        "expected about {STEER_RATE_TOP_SPEED_RAD_PER_S} rad/s, measured {rate}"
    );
}

#[test]
fn full_brake_from_top_speed_stops_in_the_tuned_time() {
    let at_top_speed = run(
        race(1),
        vec![accelerate(); ticks(ACCELERATION_TO_TOP_S) + 1],
    );
    // A tick moves the kart at the speed it reaches that tick, so the last
    // tick before the tuned time is the last one that still rolls forward.
    let nearly_stopped = run(at_top_speed, vec![brake(); ticks(BRAKE_TO_STOP_S) - 2]);
    let before = only_kart(&nearly_stopped);
    let last_rolling = step(nearly_stopped, &[brake()], DT);
    assert!(
        forward_speed(&before, &only_kart(&last_rolling)) > 0.1,
        "still rolling forward one tick before the tuned time"
    );

    let before = only_kart(&last_rolling);
    let after = only_kart(&step(last_rolling, &[brake()], DT));
    assert!(forward_speed(&before, &after).abs() < 0.5);
}

#[test]
fn holding_the_brake_past_a_stop_reverses_up_to_the_tuned_reverse_top_speed() {
    let spawn = only_kart(&race(1));
    let reversing = run(race(1), vec![brake(); ticks(2.0)]);
    let before = only_kart(&reversing);
    let after = only_kart(&step(reversing, &[brake()], DT));

    assert!(
        after.position[2] > spawn.position[2] + 1.0,
        "moved backward (+Z)"
    );
    let speed = forward_speed(&before, &after);
    assert!(
        (speed + REVERSE_TOP_SPEED_M_PER_S).abs() < 0.5,
        "expected about -{REVERSE_TOP_SPEED_M_PER_S} m/s, measured {speed}"
    );
    assert_eq!(
        after.speed, -REVERSE_TOP_SPEED_M_PER_S,
        "the readout is signed"
    );
}

#[test]
fn brake_with_the_throttle_held_stops_the_kart_and_holds_it_there() {
    let both = KartInput {
        brake: 1.0,
        ..accelerate()
    };
    let at_top_speed = run(race(1), vec![accelerate(); ticks(ACCELERATION_TO_TOP_S)]);
    // The throttle fights the brake, so this takes longer than the tuned
    // braking time and ends within the 100 m the straight stays clear.
    let stopped = run(at_top_speed, vec![both; ticks(2.0 * ACCELERATION_TO_TOP_S)]);
    let before = only_kart(&stopped);
    let after = only_kart(&step(stopped, &[both], DT));

    assert!(distance_travelled(&before, &after) / DT < 0.01);
    assert_eq!(after.speed, 0.0, "held at the standstill, not backing up");
}

#[test]
fn releasing_the_brake_while_reversing_coasts_to_a_stop() {
    let reversing = run(race(1), vec![brake(); ticks(1.0)]);
    let coasted = run(
        reversing,
        vec![KartInput::default(); ticks(COAST_TO_STOP_S)],
    );
    let before = only_kart(&coasted);
    let after = only_kart(&step(coasted, &[KartInput::default()], DT));

    assert!(distance_travelled(&before, &after) / DT < 0.01);
    assert_eq!(after.speed, 0.0);
}

#[test]
fn throttle_while_backing_up_drives_the_kart_forward_again() {
    let spawn = only_kart(&race(1));
    let reversing = run(race(1), vec![brake(); ticks(1.0)]);
    let behind = only_kart(&reversing);
    assert!(
        behind.position[2] > spawn.position[2] + 1.0,
        "backed up first"
    );

    let recovering = run(reversing, vec![accelerate(); ticks(1.0)]);
    let before = only_kart(&recovering);
    let after = only_kart(&step(recovering, &[accelerate()], DT));
    assert!(forward_speed(&before, &after) > 1.0);
    assert!(after.speed > 0.0);
}

#[test]
fn steering_while_reversing_swings_the_nose_the_other_way() {
    let reverse_right = KartInput {
        steer: 1.0,
        ..brake()
    };
    let reversing = run(race(1), vec![brake(); ticks(1.0)]);

    // Forward, steer right turns clockwise (negative yaw); backing up with
    // the same steer swings the nose anticlockwise, as a car does.
    assert!(measured_yaw_rate(reversing, reverse_right) > 0.5);
}

#[test]
fn releasing_the_throttle_coasts_to_a_stop_in_the_tuned_time() {
    let at_top_speed = run(race(1), vec![accelerate(); ticks(ACCELERATION_TO_TOP_S)]);
    let halfway = run(
        at_top_speed,
        vec![KartInput::default(); ticks(COAST_TO_STOP_S / 2.0)],
    );
    let before = only_kart(&halfway);
    let still_rolling = step(halfway, &[KartInput::default()], DT);
    assert!(
        distance_travelled(&before, &only_kart(&still_rolling)) / DT > 1.0,
        "halfway through the coast the kart is still moving"
    );

    let stopped = run(
        still_rolling,
        vec![KartInput::default(); ticks(COAST_TO_STOP_S / 2.0) + 1],
    );
    let before = only_kart(&stopped);
    let after = only_kart(&step(stopped, &[KartInput::default()], DT));
    assert!(distance_travelled(&before, &after) / DT < 0.01);
}

#[test]
fn each_kart_gets_its_own_input_and_transform() {
    let mut state = race(2);
    for _ in 0..ticks(1.0) {
        state = step(state, &[accelerate(), KartInput::default()], DT);
    }
    let spawn = race(2).kart_transforms();
    let after = state.kart_transforms();

    assert_eq!(after.len(), 2);
    assert!(after[0].position[2] < spawn[0].position[2] - 1.0);
    assert!(distance_travelled(&spawn[1], &after[1]) < 0.01);
}
