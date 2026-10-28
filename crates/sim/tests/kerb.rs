//! The sausage kerb at the sim's public seam: one kart on the oval, its
//! walls taken away, with a kerb laid where the kart is about to be, and
//! eight bots racing Brands Patch past the kerb at its chicane. Each kerb
//! test runs the kart twice, with the kerb and without, so the kerb's part
//! is the difference; the sim is deterministic and the kerb touches nothing
//! until the kart reaches it.

mod common;

use common::{
    brands_patch, only_kart, started, throttle, ticks, track, without_walls, yaw, DT, OVAL, SEED,
};

use sim::tuning::{
    ACCELERATION_TO_TOP_S, HOP_AIRTIME_S, KERB_HALF_LENGTH_M, KERB_HALF_WIDTH_M, KERB_SPEED_KEEP,
    LAPS_PER_RACE, MINI_TURBO_RED_CHARGE_S, TOP_SPEED_M_PER_S,
};
use sim::{step, KartInput, KartReadout, MiniTurbo, Phase, Placement, State};

/// The oval without its walls, with `kerbs` laid on it and none of its own.
fn race_with_kerbs(kerbs: &[Placement]) -> State {
    let mut sidecar: serde_json::Value =
        serde_json::from_str(&without_walls(OVAL)).expect("valid JSON");
    sidecar["kerbs"] = kerbs
        .iter()
        .map(|kerb| serde_json::json!({ "position": kerb.position, "yaw": kerb.yaw }))
        .collect();
    started(State::new(SEED, 1, track(&sidecar.to_string())))
}

/// The kart's readout after each of `inputs`, from the start of a race on
/// the oval with `kerbs`.
fn readouts_over(kerbs: &[Placement], inputs: &[KartInput]) -> Vec<KartReadout> {
    let mut state = race_with_kerbs(kerbs);
    let mut readouts = Vec::with_capacity(inputs.len());
    for input in inputs {
        state = step(state, &[*input], DT);
        readouts.push(only_kart(&state));
    }
    readouts
}

/// A kerb lying along the kart's travel as `readout` reports it, centred
/// `ahead` metres in front of the kart and `right` metres to its right.
fn kerb_ahead_of(readout: &KartReadout, ahead: f32, right: f32) -> Placement {
    let yaw = yaw(readout.rotation);
    // Yaw zero faces -Z; forward is (-sin, -cos) and right (cos, -sin) in x, z.
    let (sin, cos) = yaw.sin_cos();
    let [x, y, z] = readout.position;
    Placement {
        position: [
            x - ahead * sin + right * cos,
            y,
            z - ahead * cos - right * sin,
        ],
        yaw,
    }
}

/// Full throttle to top speed, then `then`, held for long enough to cross a
/// kerb laid 15 m ahead and land well past it.
fn to_top_speed_then(then: KartInput) -> Vec<KartInput> {
    let mut inputs = vec![throttle(); ticks(ACCELERATION_TO_TOP_S) + 1];
    inputs.extend(vec![then; ticks(1.5)]);
    inputs
}

/// The readout the run to top speed ends on, where the tests lay a kerb
/// 15 m ahead of the kart.
fn top_speed_tick() -> usize {
    ticks(ACCELERATION_TO_TOP_S)
}

/// The ticks the kart spent in the air, as `(first, count)`, from its
/// readouts after tick `from`.
fn airtime(readouts: &[KartReadout], from: usize) -> Option<(usize, usize)> {
    let first = from + readouts[from..].iter().position(|r| r.airborne)?;
    let count = readouts[first..].iter().take_while(|r| r.airborne).count();
    Some((first, count))
}

#[test]
fn a_kart_driven_onto_the_kerb_leaves_the_ground_for_the_hops_airtime_and_lands_with_the_kept_share_of_its_speed(
) {
    let inputs = to_top_speed_then(throttle());
    let clear = readouts_over(&[], &inputs);
    let from = top_speed_tick();
    assert_eq!(clear[from].speed, TOP_SPEED_M_PER_S, "at top speed");
    let kerb = kerb_ahead_of(&clear[from], 15.0, 0.0);

    let hopped = readouts_over(&[], &to_top_speed_then(hop()));
    let (_, hop_ticks) = airtime(&hopped, from).expect("the kart hopped");

    let over = readouts_over(&[kerb], &inputs);
    let (first, count) = airtime(&over, from).expect("the kerb launched the kart");
    assert_eq!(count, hop_ticks, "in the air for the hop's airtime");
    assert_eq!(
        over[first - 1].speed,
        TOP_SPEED_M_PER_S,
        "at top speed the tick before"
    );
    assert_eq!(
        over[first].speed,
        KERB_SPEED_KEEP * TOP_SPEED_M_PER_S,
        "the launch keeps its share of the speed"
    );
    // Held through the hop, the throttle ramps the speed back up as in any hop.
    let regained = count as f32 * DT * TOP_SPEED_M_PER_S / ACCELERATION_TO_TOP_S;
    let landed = over[first + count];
    assert!(!landed.airborne);
    assert!(
        (landed.speed - (KERB_SPEED_KEEP * TOP_SPEED_M_PER_S + regained)).abs() < 1e-3,
        "lands with the kept share and the throttle's ramp: {}",
        landed.speed
    );
    assert_eq!(landed.drift, None, "lands straight");
}

/// The hop button pressed with the throttle down and the wheel straight.
fn hop() -> KartInput {
    KartInput {
        hop: true,
        ..throttle()
    }
}

/// Hop and steer right with the throttle down: held, a right drift.
fn hop_right() -> KartInput {
    KartInput {
        hop: true,
        steer: 1.0,
        ..throttle()
    }
}

#[test]
fn a_kart_drifting_across_the_kerb_lands_with_no_drift_and_no_mini_turbo_however_long_it_charged() {
    // A drift held past red, then a kerb laid where the kart is going; the
    // hop button stays down through the kerb and is let go after landing.
    let charged = ticks(HOP_AIRTIME_S) + 2 + ticks(MINI_TURBO_RED_CHARGE_S) + 6;
    let mut inputs = vec![throttle(); ticks(ACCELERATION_TO_TOP_S) + 1];
    inputs.extend(vec![hop_right(); charged + ticks(1.0)]);
    inputs.extend(vec![throttle(); ticks(1.0)]);
    let clear = readouts_over(&[], &inputs);
    let reach = ticks(ACCELERATION_TO_TOP_S) + 1 + charged;
    assert_eq!(clear[reach].mini_turbo, MiniTurbo::Red, "charged to red");
    // Centred a little way along the drift's path, which curves right: the
    // kerb lies along the kart's travel where it will be, its near end
    // reached after `reach`.
    let ahead = &clear[reach + 12];
    let kerb = Placement {
        position: ahead.position,
        yaw: (-ahead.velocity[0]).atan2(-ahead.velocity[2]),
    };

    let over = readouts_over(&[kerb], &inputs);
    let (first, count) = airtime(&over, reach).expect("the kerb launched the kart");
    assert!(over[first - 1].drift.is_some(), "drifting onto the kerb");
    assert_eq!(over[first - 1].mini_turbo, MiniTurbo::Red);
    assert!((ticks(HOP_AIRTIME_S)..=ticks(HOP_AIRTIME_S) + 1).contains(&count));
    for readout in &over[first..first + count] {
        assert_eq!(readout.drift, None, "the drift ends at the launch");
        assert_eq!(readout.mini_turbo, MiniTurbo::None, "no sparks in the air");
    }
    for readout in &over[first + count..] {
        assert_eq!(readout.drift, None, "lands with no drift");
        assert_eq!(
            readout.boost_s, None,
            "and no mini-turbo, nor on the release"
        );
    }
    let released = clear[reach..]
        .iter()
        .any(|r| r.boost_tier == MiniTurbo::Red);
    assert!(
        released,
        "without the kerb the same inputs release a red boost"
    );
}

#[test]
fn a_kart_in_the_air_over_the_kerb_is_not_launched_and_keeps_its_drift() {
    // A hop right at top speed; the kerb lies across the kart's path where
    // it is halfway through the hop, so the kart is over it only in the air.
    let mut inputs = vec![throttle(); ticks(ACCELERATION_TO_TOP_S) + 1];
    inputs.extend(vec![hop_right(); ticks(1.5)]);
    let clear = readouts_over(&[], &inputs);
    let hop = ticks(ACCELERATION_TO_TOP_S) + 1;
    let midair = &clear[hop + ticks(HOP_AIRTIME_S / 2.0)];
    assert!(midair.airborne);
    let across = kerb_ahead_of(midair, 0.0, 0.0);
    let kerb = Placement {
        yaw: across.yaw + std::f32::consts::FRAC_PI_2,
        ..across
    };

    let over = readouts_over(&[kerb], &inputs);
    assert_eq!(over, clear, "the kerb changes nothing");
    assert!(
        over.last().expect("readouts").drift.is_some(),
        "landed into the drift"
    );
}

#[test]
fn a_kart_driving_beside_the_kerb_is_untouched() {
    let inputs = to_top_speed_then(throttle());
    let clear = readouts_over(&[], &inputs);
    let beside = KERB_HALF_WIDTH_M + 0.1;
    for right in [beside, -beside] {
        let kerb = kerb_ahead_of(&clear[top_speed_tick()], 15.0, right);
        assert_eq!(
            readouts_over(&[kerb], &inputs),
            clear,
            "a kerb {right} m right"
        );
    }
}

#[test]
fn a_slow_kart_that_lands_back_on_the_kerb_is_not_launched_again_until_it_has_left_it() {
    // From the grid, a kerb whose near end is 2 m ahead: the kart reaches
    // it slowly enough to land on it and drive the rest of its length.
    let inputs = vec![throttle(); ticks(3.0)];
    let clear = readouts_over(&[], &inputs);
    let kerb = kerb_ahead_of(&clear[0], 2.0 + KERB_HALF_LENGTH_M, 0.0);

    let over = readouts_over(&[kerb], &inputs);
    let (first, count) = airtime(&over, 0).expect("the kerb launched the kart");
    let landed = first + count;
    assert!(
        race_with_kerbs(&[kerb])
            .track()
            .on_a_kerb(over[landed].position),
        "landed on the kerb: {:?}",
        over[landed]
    );
    assert_eq!(airtime(&over, landed), None, "launched once");
    // Back round the oval to the kerb would take far longer than this run.
    let far_end = kerb_ahead_of(&clear[0], 2.0 + 2.0 * KERB_HALF_LENGTH_M, 0.0);
    let (past, _) = far_end.local(over.last().expect("readouts").position);
    assert!(past > 0.0, "drove off the kerb's far end");
}

#[test]
fn eight_bots_racing_three_laps_of_brands_patch_are_never_launched_by_the_kerb() {
    let track = brands_patch();
    assert_eq!(track.kerbs().len(), 1, "the chicane's kerb");
    // Bots braking for the runaway wheel queue back into the chicane, where
    // traffic can shove one onto the kerb on some seeds; this one races
    // clear of that, so it shows the bots' own line keeps off the kerb.
    let mut state = started(State::with_bots(11, 0, 8, track));
    for _ in 0..ticks(LAPS_PER_RACE as f32 * 60.0) {
        state = step(state, &[], DT);
        for (slot, kart) in state.kart_readouts().iter().enumerate() {
            assert!(
                !state.track().on_a_kerb(kart.position),
                "bot {slot} reached the kerb: {kart:?}"
            );
        }
        if state.phase() == Phase::Finished
            || state
                .standings()
                .iter()
                .all(|s| s.laps_completed >= LAPS_PER_RACE)
        {
            break;
        }
    }
    assert!(
        state
            .standings()
            .iter()
            .all(|s| s.laps_completed >= LAPS_PER_RACE),
        "every bot raced its three laps"
    );
}
