//! The bridge's surface as the web app sees it: create a race on a sidecar,
//! step with a flat input buffer, read a flat kart buffer. Native tests; the
//! wasm build of the same code is checked in CI.

use bridge::Race;
use sim::tuning::{KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M, TICKS_PER_SECOND};
use sim::{Surface, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
/// Floats per kart in the kart buffer: position xyz, rotation xyzw, speed, surface code.
const FLOATS_PER_KART: usize = 9;
/// Floats per kart in the input buffer: steer, throttle, brake, hop, use item, hold item.
const FLOATS_PER_INPUT: usize = 6;

fn race(seed: u64, kart_count: usize) -> Race {
    Race::new(seed, kart_count, OVAL).expect("the oval fixture loads")
}

#[test]
fn kart_buffer_holds_nine_floats_per_kart_on_the_spawn_grid_in_slot_order() {
    let race = race(7, 3);
    let buffer = race.kart_transforms();
    let track = Track::from_sidecar(OVAL).unwrap();

    assert_eq!(buffer.len(), 3 * FLOATS_PER_KART);
    for (kart, slot) in buffer.chunks(FLOATS_PER_KART).zip(track.spawn_grid()) {
        assert_eq!(kart[0], slot.position[0]);
        assert!((kart[1] - slot.position[1] - KART_HALF_HEIGHT_M).abs() < 1e-5);
        assert_eq!(kart[2], slot.position[2]);
        assert_eq!(
            kart[3..7],
            [0.0, 0.0, 0.0, 1.0],
            "the oval's grid faces -Z, so karts spawn with identity rotation"
        );
        assert_eq!(kart[7], 0.0, "karts spawn at rest");
        assert_eq!(
            kart[8],
            f32::from(Surface::Asphalt.code()),
            "karts spawn on asphalt"
        );
    }
}

#[test]
fn the_kart_size_is_the_tuned_box() {
    assert_eq!(
        bridge::kart_size_m(),
        vec![
            2.0 * KART_HALF_WIDTH_M,
            2.0 * KART_HALF_HEIGHT_M,
            2.0 * KART_HALF_LENGTH_M
        ]
    );
}

#[test]
fn an_unusable_sidecar_or_too_many_karts_is_an_error_not_a_panic() {
    assert!(Race::new(7, 1, "not json").is_err());
    assert!(Race::new(7, 1, &OVAL.replacen("\"version\":1", "\"version\":9", 1)).is_err());
    assert!(
        Race::new(7, 9, OVAL).is_err(),
        "the oval has eight spawn slots"
    );
}

#[test]
fn step_applies_each_karts_input_from_the_flat_buffer() {
    let mut race = race(7, 2);
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
fn the_buffer_is_the_sim_crates_readouts_for_the_same_race() {
    let input = sim::KartInput {
        steer: 0.5,
        throttle: 1.0,
        ..sim::KartInput::default()
    };
    let mut state = sim::State::new(42, 1, Track::from_sidecar(OVAL).unwrap());
    let mut race = race(42, 1);
    // Long enough to steer off the asphalt, so the surface code changes too.
    for _ in 0..3 * TICKS_PER_SECOND {
        state = sim::step(state, &[input], DT);
        race.step(
            &[input.steer, input.throttle, input.brake, 0.0, 0.0, 0.0],
            DT,
        );
    }

    let expected = state.kart_transforms()[0];
    let buffer = race.kart_transforms();
    assert_eq!(buffer[..3], expected.position);
    assert_eq!(buffer[3..7], expected.rotation);
    assert_eq!(buffer[7], expected.speed);
    assert_ne!(
        state.kart_surface(0),
        Surface::Asphalt,
        "the kart left the asphalt"
    );
    assert_eq!(buffer[8], f32::from(state.kart_surface(0).code()));
}

#[test]
fn a_short_input_buffer_is_rejected_and_the_race_stays_usable() {
    let mut race = race(7, 2);
    // Three floats per kart was the buffer before the buttons; it is short now.
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        race.step(&[0.0, 1.0, 0.0, 0.0, 1.0, 0.0], DT);
    }));
    assert!(rejected.is_err(), "a short buffer must be rejected");

    race.step(&[0.0; 2 * FLOATS_PER_INPUT], DT);
    assert_eq!(race.kart_transforms().len(), 2 * FLOATS_PER_KART);
}

#[test]
fn the_state_hash_is_the_sim_crates_hash_for_the_same_race() {
    let mut state = sim::State::new(42, 2, Track::from_sidecar(OVAL).unwrap());
    let mut race = race(42, 2);
    assert_eq!(race.state_hash(), state.hash());
    let turn = sim::KartInput {
        steer: 1.0,
        throttle: 1.0,
        ..sim::KartInput::default()
    };
    let straight = sim::KartInput { steer: 0.0, ..turn };
    for _ in 0..TICKS_PER_SECOND {
        state = sim::step(state, &[turn, straight], DT);
        race.step(
            &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            DT,
        );
    }
    assert_eq!(race.state_hash(), state.hash());
}

#[test]
fn replay_hashes_runs_a_replay_json_on_a_sidecar_and_reports_the_sims_hashes() {
    const REPLAY: &str = include_str!("../../../assets/replays/oval-solo.replay.json");
    let replay = sim::Replay::from_json(REPLAY).unwrap();
    let expected = replay.run(Track::from_sidecar(OVAL).unwrap()).unwrap();
    assert_eq!(bridge::replay_hashes(REPLAY, OVAL), Ok(expected));
    assert!(bridge::replay_hashes("{}", OVAL).is_err());
    assert!(
        bridge::replay_hashes(&REPLAY.replace("\"oval\"", "\"brands-patch\""), OVAL).is_err(),
        "the sidecar must be the replay's track"
    );
}
