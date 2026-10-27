//! The bridge's surface as the web app sees it: create a race on a sidecar,
//! step with a flat input buffer, read the karts' transforms as a flat
//! buffer and the rest of their readouts beside it. Native tests; the wasm
//! build of the same code is checked in CI.

use bridge::{Race, FLOATS_PER_TRANSFORM};
use sim::tuning::{
    HOP_AIRTIME_S, KART_HALF_HEIGHT_M, MINI_TURBO_ORANGE_CHARGE_S, TICKS_PER_SECOND,
};
use sim::{DriftDirection, KartInput, KartReadout, MiniTurbo, Surface, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
/// Floats per kart in the input buffer: steer, throttle, brake, hop, use item, hold item.
const FLOATS_PER_INPUT: usize = 6;
/// Phase codes in the phase buffer.
const COUNTDOWN: f32 = 0.0;
const RACING: f32 = 1.0;
const OUTRO: f32 = 2.0;
const FINISHED: f32 = 3.0;

fn race(seed: u64, kart_count: usize) -> Race {
    Race::new(seed, kart_count, 0, OVAL).expect("the oval fixture loads")
}

/// Step the race idle through its countdown.
fn go(race: &mut Race, humans: usize) {
    let idle = vec![0.0; humans * FLOATS_PER_INPUT];
    while race.phase()[0] == COUNTDOWN {
        race.step(&idle, DT);
    }
}

/// The sim's race past its countdown, every kart idle on the grid until Go.
fn started(mut state: sim::State) -> sim::State {
    let inputs = vec![sim::KartInput::default(); state.human_count()];
    while matches!(state.phase(), sim::Phase::Countdown { .. }) {
        state = sim::step(state, &inputs, DT);
    }
    state
}

/// The input buffer `Race::step` documents, for one input per human kart.
fn floats(inputs: &[KartInput]) -> Vec<f32> {
    inputs
        .iter()
        .flat_map(|input| {
            [
                input.steer,
                input.throttle,
                input.brake,
                f32::from(u8::from(input.hop)),
                f32::from(u8::from(input.use_item)),
                f32::from(u8::from(input.hold_item)),
            ]
        })
        .collect()
}

/// The sim crate's readouts for `state`, each with the surface under it:
/// what the bridge's race should report for the same race.
fn sims_readouts(state: &sim::State) -> Vec<(KartReadout, Surface)> {
    state
        .kart_readouts()
        .into_iter()
        .enumerate()
        .map(|(slot, kart)| (kart, state.kart_surface(slot)))
        .collect()
}

/// A race with `humans` humans and `bots` bots, past its countdown.
fn started_race(seed: u64, humans: usize, bots: usize) -> Race {
    let mut race = Race::new(seed, humans, bots, OVAL).expect("the karts fit the oval");
    go(&mut race, humans);
    race
}

#[test]
fn karts_spawn_on_the_spawn_grid_in_slot_order_at_rest_on_asphalt() {
    let race = race(7, 3);
    let track = Track::from_sidecar(OVAL).unwrap();
    let karts = race.kart_readouts();

    assert_eq!(karts.len(), 3);
    for ((kart, surface), slot) in karts.iter().zip(track.spawn_grid()) {
        assert_eq!(kart.position[0], slot.position[0]);
        assert!((kart.position[1] - slot.position[1] - KART_HALF_HEIGHT_M).abs() < 1e-5);
        assert_eq!(kart.position[2], slot.position[2]);
        assert_eq!(
            kart.rotation,
            [0.0, 0.0, 0.0, 1.0],
            "the oval's grid faces -Z, so karts spawn with identity rotation"
        );
        assert_eq!(kart.speed, 0.0, "karts spawn at rest");
        assert_eq!(*surface, Surface::Asphalt, "karts spawn on asphalt");
        assert_eq!(kart.drift, None, "karts spawn out of a drift");
        assert_eq!(kart.held_item, None, "karts spawn with an empty item slot");
        assert_eq!(kart.spin_out_s, None, "karts spawn out of a spin-out");
        assert!(!kart.airborne, "karts spawn on the ground");
        assert_eq!(kart.boost_s, None, "karts spawn without a boost");
    }
}

#[test]
fn the_transform_buffer_is_each_karts_position_then_rotation_in_grid_order() {
    let mut race = started_race(7, 2, 0);
    let turn = KartInput {
        steer: 1.0,
        throttle: 1.0,
        ..KartInput::default()
    };
    for _ in 0..TICKS_PER_SECOND {
        race.step(&floats(&[turn, KartInput::default()]), DT);
    }

    let buffer = race.kart_transforms();
    let karts = race.kart_readouts();
    assert_eq!(buffer.len(), karts.len() * FLOATS_PER_TRANSFORM);
    for (floats, (kart, _)) in buffer.chunks(FLOATS_PER_TRANSFORM).zip(&karts) {
        let transform: Vec<f32> = kart.position.into_iter().chain(kart.rotation).collect();
        assert_eq!(floats, transform);
    }
}

#[test]
fn the_readouts_report_a_drift_its_direction_tier_and_charge_as_the_sim_does() {
    let hop_left = KartInput {
        steer: -1.0,
        throttle: 1.0,
        hop: true,
        ..KartInput::default()
    };
    let mut state = started(sim::State::new(42, 1, Track::from_sidecar(OVAL).unwrap()));
    let mut race = started_race(42, 1, 0);
    // Up to speed, then a hop into a left drift held just past orange.
    let to_speed = 3 * TICKS_PER_SECOND;
    let hold =
        ((HOP_AIRTIME_S + MINI_TURBO_ORANGE_CHARGE_S + 0.1) * TICKS_PER_SECOND as f32) as u32;
    for tick in 0..to_speed + hold {
        let input = if tick < to_speed {
            KartInput {
                steer: 0.0,
                hop: false,
                ..hop_left
            }
        } else {
            hop_left
        };
        state = sim::step(state, &[input], DT);
        race.step(&floats(&[input]), DT);
        if tick == to_speed {
            assert!(
                race.kart_readouts()[0].0.airborne,
                "in the air from the hop"
            );
        }
    }

    let (kart, _) = race.kart_readouts()[0];
    let drift = kart.drift.expect("the bridge's race is drifting");
    assert_eq!(drift.direction, DriftDirection::Left);
    assert_eq!(kart.mini_turbo, MiniTurbo::Orange);
    assert!(!kart.airborne, "landed into the drift");
    assert_eq!(race.kart_readouts(), sims_readouts(&state));
}

#[test]
fn an_unusable_sidecar_or_too_many_karts_is_an_error_not_a_panic() {
    assert!(Race::new(7, 1, 0, "not json").is_err());
    assert!(Race::new(7, 1, 0, &OVAL.replacen("\"version\":1", "\"version\":9", 1)).is_err());
    assert!(
        Race::new(7, 9, 0, OVAL).is_err(),
        "the oval has eight spawn slots"
    );
}

#[test]
fn step_applies_each_karts_input_from_the_flat_buffer() {
    let mut race = started_race(7, 2, 0);
    let spawn = race.kart_readouts();
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    for _ in 0..TICKS_PER_SECOND {
        race.step(&floats(&[throttle, KartInput::default()]), DT);
    }
    let after = race.kart_readouts();

    assert!(
        after[0].0.position[2] < spawn[0].0.position[2] - 1.0,
        "kart 0 drove forward (-Z)"
    );
    for (after, spawn) in after[1].0.position.iter().zip(spawn[1].0.position) {
        assert!((after - spawn).abs() < 0.01, "kart 1 stayed put");
    }
}

#[test]
fn the_readouts_are_the_sim_crates_readouts_for_the_same_race() {
    let input = KartInput {
        steer: 0.5,
        throttle: 1.0,
        ..KartInput::default()
    };
    let mut state = started(sim::State::new(42, 1, Track::from_sidecar(OVAL).unwrap()));
    let mut race = started_race(42, 1, 0);
    // Long enough to steer off the asphalt, so the surface changes too.
    for _ in 0..3 * TICKS_PER_SECOND {
        state = sim::step(state, &[input], DT);
        race.step(&floats(&[input]), DT);
    }

    assert_ne!(
        state.kart_surface(0),
        Surface::Asphalt,
        "the kart left the asphalt"
    );
    assert_eq!(race.kart_readouts(), sims_readouts(&state));
}

#[test]
fn the_standings_are_the_sim_crates_standings_for_the_same_race() {
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let mut state = started(sim::State::new(42, 2, Track::from_sidecar(OVAL).unwrap()));
    let mut race = started_race(42, 2, 0);
    // Long enough for the second kart to pass pole and the first checkpoint.
    for _ in 0..3 * TICKS_PER_SECOND {
        state = sim::step(
            state,
            &[
                sim::KartInput::default(),
                sim::KartInput {
                    throttle: 1.0,
                    ..sim::KartInput::default()
                },
            ],
            DT,
        );
        race.step(&floats(&[KartInput::default(), throttle]), DT);
    }

    let standings = race.standings();
    assert_eq!(standings, state.standings());
    assert_eq!(standings[0].position, 2, "pole was passed");
    assert_eq!(standings[1].position, 1);
    assert_eq!(standings[1].checkpoint, 1, "past the first gate");
}

#[test]
fn the_readouts_flag_a_kart_going_the_wrong_way() {
    use sim::tuning::WRONG_WAY_WARNING_S;
    let brake = KartInput {
        brake: 1.0,
        ..KartInput::default()
    };
    let mut race = started_race(42, 1, 0);
    // Backing away from the line for longer than the warning takes.
    for _ in 0..(WRONG_WAY_WARNING_S * TICKS_PER_SECOND as f32) as u32 + 2 {
        race.step(&floats(&[brake]), DT);
    }
    assert!(race.kart_readouts()[0].0.wrong_way, "wrong way");
}

#[test]
fn a_short_input_buffer_is_rejected_and_the_race_stays_usable() {
    let mut race = race(7, 2);
    // One kart's worth of floats for a two-kart race.
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        race.step(&[0.0, 1.0, 0.0, 0.0, 1.0, 0.0], DT);
    }));
    assert!(rejected.is_err(), "a short buffer must be rejected");

    race.step(&[0.0; 2 * FLOATS_PER_INPUT], DT);
    assert_eq!(race.kart_readouts().len(), 2);
}

#[test]
fn the_state_hash_is_the_sim_crates_hash_for_the_same_race() {
    let mut state = started(sim::State::new(42, 2, Track::from_sidecar(OVAL).unwrap()));
    let mut race = started_race(42, 2, 0);
    assert_eq!(race.state_hash(), state.hash());
    let turn = KartInput {
        steer: 1.0,
        throttle: 1.0,
        ..KartInput::default()
    };
    let straight = KartInput { steer: 0.0, ..turn };
    for _ in 0..TICKS_PER_SECOND {
        state = sim::step(state, &[turn, straight], DT);
        race.step(&floats(&[turn, straight]), DT);
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

#[test]
fn bots_fill_the_slots_behind_the_humans_and_step_takes_only_the_humans_inputs() {
    // A seed that rolls neither bot the start boost, so both pass the idle
    // human instead of shoving it along.
    let mut race = started_race(4, 1, 2);
    assert_eq!(race.kart_readouts().len(), 3);
    assert_eq!(race.standings().len(), 3);
    let idle = [0.0; FLOATS_PER_INPUT];
    for _ in 0..3 * TICKS_PER_SECOND {
        race.step(&idle, DT);
    }
    let karts = race.kart_readouts();
    // Both bots drove past the idle human: it is last.
    assert_eq!(karts[0].0.race_position, 3);
    assert!(karts[2].0.speed > 20.0, "the second bot is at speed");
    assert!(
        Race::new(7, 1, 8, OVAL).is_err(),
        "nine karts do not fit the oval's grid"
    );
}

#[test]
fn the_phase_buffer_walks_countdown_racing_outro_and_finished_as_the_sim_does() {
    use sim::tuning::{COUNTDOWN_S, RACE_END_CAP_S, RACE_OUTRO_S};
    // With no humans, the bots' own finishes start the cap and end the race.
    let mut race = Race::new(42, 0, 2, OVAL).expect("two bots fit the oval");
    assert_eq!(race.phase(), vec![COUNTDOWN, COUNTDOWN_S, -1.0]);
    go(&mut race, 0);
    assert_eq!(race.phase(), vec![RACING, 0.0, -1.0]);

    // The sim's race is stepped alongside to say what the buffers should hold.
    let mut state = started(sim::State::with_bots(
        42,
        0,
        2,
        Track::from_sidecar(OVAL).unwrap(),
    ));
    let finished = |state: &sim::State| {
        state
            .standings()
            .iter()
            .filter(|standing| standing.finish_time_s.is_some())
            .count()
    };
    let mut ticks = 0;
    while finished(&state) == 0 {
        state = sim::step(state, &[], DT);
        race.step(&[], DT);
        ticks += 1;
        assert!(ticks < 90 * TICKS_PER_SECOND, "three laps take under 90 s");
    }
    let phase = race.phase();
    assert_eq!(phase[0], RACING);
    assert!(
        (phase[1] - ticks as f32 * DT).abs() < 0.01,
        "race clock {}",
        phase[1]
    );
    assert!((phase[2] - RACE_END_CAP_S).abs() < 1e-5, "cap {}", phase[2]);
    assert_eq!(race.standings(), state.standings());

    while finished(&state) < 2 {
        state = sim::step(state, &[], DT);
        race.step(&[], DT);
    }
    assert_eq!(race.phase(), vec![OUTRO, RACE_OUTRO_S, -1.0]);

    // Through the outro, with a tick for the timer's rounding.
    for _ in 0..(RACE_OUTRO_S * TICKS_PER_SECOND as f32) as u32 + 1 {
        race.step(&[], DT);
    }
    assert_eq!(race.phase(), vec![FINISHED, 0.0, -1.0]);
    assert_eq!(
        race.standings()[0].finish_time_s,
        state.standings()[0].finish_time_s
    );
}
