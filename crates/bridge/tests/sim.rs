//! The bridge's surface as the web app sees it: create, step with a flat
//! input buffer, read a flat transform buffer. Native tests; the wasm build
//! of the same code is checked in CI.

use bridge::Race;
use sim::tuning::TICKS_PER_SECOND;

const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
/// Floats per kart in the transform buffer: position xyz, rotation xyzw, speed.
const FLOATS_PER_KART: usize = 8;
/// Floats per kart in the input buffer: steer, throttle, brake.
const FLOATS_PER_INPUT: usize = 3;

#[test]
fn transform_buffer_holds_seven_floats_per_kart_in_grid_order() {
    let race = Race::new(7, 3);
    let buffer = race.kart_transforms();

    assert_eq!(buffer.len(), 3 * FLOATS_PER_KART);
    let xs: Vec<f32> = buffer.chunks(FLOATS_PER_KART).map(|kart| kart[0]).collect();
    assert!(
        xs[0] < xs[1] && xs[1] < xs[2],
        "grid slots run left to right: {xs:?}"
    );
    for kart in buffer.chunks(FLOATS_PER_KART) {
        assert_eq!(
            kart[3..7],
            [0.0, 0.0, 0.0, 1.0],
            "karts spawn with identity rotation"
        );
        assert_eq!(kart[7], 0.0, "karts spawn at rest");
    }
}

#[test]
fn step_applies_each_karts_input_from_the_flat_buffer() {
    let mut race = Race::new(7, 2);
    let spawn = race.kart_transforms();
    let mut inputs = vec![0.0; 2 * FLOATS_PER_INPUT];
    inputs[1] = 1.0; // kart 0: full throttle, kart 1: nothing
    for _ in 0..TICKS_PER_SECOND {
        race.step(&inputs, DT);
    }
    let after = race.kart_transforms();

    assert!(after[2] < spawn[2] - 1.0, "kart 0 drove forward (-Z)");
    for (after, spawn) in after[FLOATS_PER_KART..]
        .iter()
        .zip(&spawn[FLOATS_PER_KART..])
    {
        assert!((after - spawn).abs() < 0.01, "kart 1 stayed put");
    }
}

#[test]
fn the_buffer_is_the_sim_crates_transforms_for_the_same_race() {
    let input = sim::KartInput {
        steer: 0.5,
        throttle: 1.0,
        brake: 0.0,
    };
    let mut state = sim::State::new(42, 1);
    let mut race = Race::new(42, 1);
    for _ in 0..TICKS_PER_SECOND {
        state = sim::step(state, &[input], DT);
        race.step(&[input.steer, input.throttle, input.brake], DT);
    }

    let expected = state.kart_transforms()[0];
    let buffer = race.kart_transforms();
    assert_eq!(buffer[..3], expected.position);
    assert_eq!(buffer[3..7], expected.rotation);
    assert_eq!(buffer[7], expected.speed);
}

#[test]
fn a_short_input_buffer_is_rejected_and_the_race_stays_usable() {
    let mut race = Race::new(7, 2);
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        race.step(&[0.0, 1.0, 0.0], DT);
    }));
    assert!(rejected.is_err(), "a short buffer must be rejected");

    race.step(&[0.0; 2 * FLOATS_PER_INPUT], DT);
    assert_eq!(race.kart_transforms().len(), 2 * FLOATS_PER_KART);
}
