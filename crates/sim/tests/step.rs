//! The sim's public seam: `step(state, inputs, dt)`. Tests drive a kart with
//! input sequences and assert the transforms the sim reports, never internals.
//! Races run on the oval fixture, whose front straight is clear for 100 m
//! past the start line and runs along -Z.

use sim::tuning::{ACCELERATION_TO_TOP_S, COAST_TO_STOP_S, TICKS_PER_SECOND, TOP_SPEED_M_PER_S};
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

fn drive_then_turn() -> Vec<KartInput> {
    let accelerate = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let turn = KartInput {
        throttle: 1.0,
        steer: 1.0,
        ..KartInput::default()
    };
    let mut inputs = vec![accelerate; ticks(3.0)];
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
    let accelerate = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    // One tick past the tuned ramp so the comparison is not on the boundary.
    let state = run(race(1), vec![accelerate; ticks(ACCELERATION_TO_TOP_S) + 1]);
    let before = only_kart(&state);
    let state = step(state, &[accelerate], DT);
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
    let accelerate = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let after = only_kart(&run(race(1), vec![accelerate; ticks(1.0)]));

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
fn brake_brings_a_moving_kart_to_a_stop() {
    let accelerate = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let brake = KartInput {
        brake: 1.0,
        ..KartInput::default()
    };
    let moving = run(race(1), vec![accelerate; ticks(1.0)]);
    let stopped = run(moving, vec![brake; ticks(3.0)]);
    let before = only_kart(&stopped);
    let after = only_kart(&step(stopped, &[brake], DT));

    assert!(distance_travelled(&before, &after) / DT < 0.01);
}

#[test]
fn releasing_the_throttle_coasts_to_a_stop_in_the_tuned_time() {
    let accelerate = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let at_top_speed = run(race(1), vec![accelerate; ticks(ACCELERATION_TO_TOP_S)]);
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
    let accelerate = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let mut state = race(2);
    for _ in 0..ticks(1.0) {
        state = step(state, &[accelerate, KartInput::default()], DT);
    }
    let spawn = race(2).kart_transforms();
    let after = state.kart_transforms();

    assert_eq!(after.len(), 2);
    assert!(after[0].position[2] < spawn[0].position[2] - 1.0);
    assert!(distance_travelled(&spawn[1], &after[1]) < 0.01);
}
