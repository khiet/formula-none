//! The bridge's surface as the web app sees it: create a race on a sidecar,
//! step with a flat input buffer, read a flat kart buffer. Native tests; the
//! wasm build of the same code is checked in CI.

use bridge::Race;
use sim::tuning::{
    HOP_AIRTIME_S, KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M, LAPS_PER_RACE,
    MINI_TURBO_BLUE_CHARGE_S, TICKS_PER_SECOND,
};
use sim::{DriftDirection, MiniTurbo, Surface, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
/// Floats per kart in the kart buffer: position xyz, rotation xyzw, speed,
/// surface code, drift direction, mini-turbo tier, drift charge.
const FLOATS_PER_KART: usize = 12;
/// Floats per kart in the input buffer: steer, throttle, brake, hop, use item, hold item.
const FLOATS_PER_INPUT: usize = 6;
/// Floats per kart in the standings buffer: position, lap, checkpoint,
/// spline progress, lap time, best lap, wrong way, finish time.
const FLOATS_PER_STANDING: usize = 8;
/// Phase codes in the phase buffer.
const COUNTDOWN: f32 = 0.0;
const RACING: f32 = 1.0;
const FINISHED: f32 = 2.0;

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

/// A race with `humans` humans and `bots` bots, past its countdown.
fn started_race(seed: u64, humans: usize, bots: usize) -> Race {
    let mut race = Race::new(seed, humans, bots, OVAL).expect("the karts fit the oval");
    go(&mut race, humans);
    race
}

#[test]
fn kart_buffer_holds_twelve_floats_per_kart_on_the_spawn_grid_in_slot_order() {
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
        assert_eq!(kart[9..12], [0.0, 0.0, 0.0], "karts spawn out of a drift");
    }
}

#[test]
fn the_buffer_reports_a_drift_its_direction_tier_and_charge_as_the_sim_does() {
    let hop_left = sim::KartInput {
        steer: -1.0,
        throttle: 1.0,
        hop: true,
        ..sim::KartInput::default()
    };
    let mut state = started(sim::State::new(42, 1, Track::from_sidecar(OVAL).unwrap()));
    let mut race = started_race(42, 1, 0);
    // Up to speed, then a hop into a left drift held just past blue.
    let to_speed = 3 * TICKS_PER_SECOND;
    let hold = ((HOP_AIRTIME_S + MINI_TURBO_BLUE_CHARGE_S + 0.1) * TICKS_PER_SECOND as f32) as u32;
    for tick in 0..to_speed + hold {
        let input = if tick < to_speed {
            sim::KartInput {
                steer: 0.0,
                hop: false,
                ..hop_left
            }
        } else {
            hop_left
        };
        state = sim::step(state, &[input], DT);
        race.step(
            &[
                input.steer,
                input.throttle,
                0.0,
                f32::from(input.hop),
                0.0,
                0.0,
            ],
            DT,
        );
    }

    let drift = state.kart_transforms()[0]
        .drift
        .expect("the sim is drifting");
    assert_eq!(drift.direction, DriftDirection::Left);
    assert_eq!(drift.mini_turbo(), MiniTurbo::Blue);
    let buffer = race.kart_transforms();
    assert_eq!(buffer[9], -1.0, "left");
    assert_eq!(buffer[10], 1.0, "blue");
    assert_eq!(buffer[11], drift.charge_s);
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
    let mut state = started(sim::State::new(42, 1, Track::from_sidecar(OVAL).unwrap()));
    let mut race = started_race(42, 1, 0);
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
fn the_standings_buffer_is_the_sim_crates_standings_for_the_same_race() {
    let throttle = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let idle = [0.0; FLOATS_PER_INPUT];
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
        race.step(&[idle, throttle].concat(), DT);
    }

    let buffer = race.standings();
    assert_eq!(buffer.len(), 2 * FLOATS_PER_STANDING);
    for (chunk, expected) in buffer.chunks(FLOATS_PER_STANDING).zip(state.standings()) {
        assert_eq!(chunk[0], expected.position as f32);
        assert_eq!(chunk[1], expected.lap() as f32);
        assert_eq!(chunk[2], expected.checkpoint as f32);
        assert_eq!(chunk[3], expected.spline_progress);
        assert_eq!(chunk[4], expected.lap_time_s);
        assert_eq!(expected.best_lap_s, None);
        assert_eq!(chunk[5], -1.0, "no best lap yet");
        assert!(!expected.wrong_way);
        assert_eq!(chunk[6], 0.0, "both face the right way");
        assert_eq!(expected.finish_time_s, None);
        assert_eq!(chunk[7], -1.0, "neither has finished");
    }
    assert_eq!(buffer[0], 2.0, "pole was passed");
    assert_eq!(buffer[FLOATS_PER_STANDING], 1.0);
    assert_eq!(buffer[FLOATS_PER_STANDING + 2], 1.0, "past the first gate");
    assert_eq!(bridge::laps_per_race(), LAPS_PER_RACE);
}

#[test]
fn the_standings_buffer_flags_a_kart_going_the_wrong_way() {
    use sim::tuning::WRONG_WAY_WARNING_S;
    let brake = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    let mut race = started_race(42, 1, 0);
    // Backing away from the line for longer than the warning takes.
    for _ in 0..(WRONG_WAY_WARNING_S * TICKS_PER_SECOND as f32) as u32 + 2 {
        race.step(&brake, DT);
    }
    assert_eq!(race.standings()[6], 1.0, "wrong way");
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
    assert_eq!(race.kart_transforms().len(), 2 * FLOATS_PER_KART);
}

#[test]
fn the_state_hash_is_the_sim_crates_hash_for_the_same_race() {
    let mut state = started(sim::State::new(42, 2, Track::from_sidecar(OVAL).unwrap()));
    let mut race = started_race(42, 2, 0);
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

#[test]
fn bots_fill_the_slots_behind_the_humans_and_step_takes_only_the_humans_inputs() {
    assert_eq!(bridge::karts_per_race(), sim::tuning::KARTS_PER_RACE);
    let mut race = started_race(7, 1, 2);
    assert_eq!(race.kart_transforms().len(), 3 * FLOATS_PER_KART);
    assert_eq!(race.standings().len(), 3 * FLOATS_PER_STANDING);
    let idle = [0.0; FLOATS_PER_INPUT];
    for _ in 0..3 * TICKS_PER_SECOND {
        race.step(&idle, DT);
    }
    // Both bots drove past the idle human: it is last.
    assert_eq!(race.standings()[0], 3.0);
    assert!(
        race.kart_transforms()[2 * FLOATS_PER_KART + 7] > 20.0,
        "the second bot is at speed"
    );
    assert!(
        Race::new(7, 1, 8, OVAL).is_err(),
        "nine karts do not fit the oval's grid"
    );
}

/// Full throttle toward a point a little right of the centreline ahead of
/// the kart: laps the oval clear of the pipes, as the sim's tests do.
fn follow_centreline(state: &sim::State, kart: usize) -> sim::KartInput {
    let transform = state.kart_transforms()[kart];
    let progress = state.standings()[kart].spline_progress;
    let track = state.track();
    let ahead = track.point_at(progress + sim::tuning::BOT_LOOKAHEAD_M);
    let further = track.point_at(progress + sim::tuning::BOT_LOOKAHEAD_M + 1.0);
    let (tx, tz) = (further[0] - ahead[0], further[2] - ahead[2]);
    let travel = (tx * tx + tz * tz).sqrt().max(f32::EPSILON);
    let target = [ahead[0] - tz / travel * 2.5, ahead[2] + tx / travel * 2.5];
    let (dx, dz) = (
        target[0] - transform.position[0],
        target[1] - transform.position[2],
    );
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    let [x, y, z, w] = transform.rotation;
    let yaw = (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z));
    let (sin, cos) = yaw.sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    sim::KartInput {
        steer: (rightward * 4.0).clamp(-1.0, 1.0),
        throttle: 1.0,
        ..sim::KartInput::default()
    }
}

#[test]
fn the_phase_buffer_walks_countdown_racing_and_finished_as_the_sim_does() {
    use sim::tuning::{COUNTDOWN_S, LAPS_PER_RACE, RACE_END_GRACE_S};
    let mut race = race(42, 2);
    assert_eq!(race.phase(), vec![COUNTDOWN, COUNTDOWN_S, -1.0]);
    go(&mut race, 2);
    assert_eq!(race.phase(), vec![RACING, 0.0, -1.0]);

    // Pole idles; the kart in the other column laps the oval, and the
    // sim's race is stepped alongside to say what the buffers should hold.
    let mut state = started(sim::State::new(42, 2, Track::from_sidecar(OVAL).unwrap()));
    let mut ticks = 0;
    while state.standings()[1].laps_completed < LAPS_PER_RACE {
        let input = follow_centreline(&state, 1);
        state = sim::step(state, &[sim::KartInput::default(), input], DT);
        race.step(
            &[
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                input.steer,
                input.throttle,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            DT,
        );
        ticks += 1;
        assert!(ticks < 90 * TICKS_PER_SECOND, "three laps take under 90 s");
    }
    let finish = state.standings()[1]
        .finish_time_s
        .expect("the lapping kart finished");
    let phase = race.phase();
    assert_eq!(phase[0], RACING);
    assert!(
        (phase[1] - ticks as f32 * DT).abs() < 0.01,
        "race clock {}",
        phase[1]
    );
    assert!(
        (phase[2] - RACE_END_GRACE_S).abs() < 1e-5,
        "grace {}",
        phase[2]
    );
    let standings = race.standings();
    assert_eq!(standings[FLOATS_PER_STANDING + 7], finish);
    assert_eq!(standings[7], -1.0, "pole has not finished");
    assert_eq!(standings[FLOATS_PER_STANDING], 1.0, "the finisher leads");

    // Idle through the grace, with a tick for the timer's rounding.
    let idle = [0.0; 2 * FLOATS_PER_INPUT];
    for _ in 0..(RACE_END_GRACE_S * TICKS_PER_SECOND as f32) as u32 + 1 {
        race.step(&idle, DT);
    }
    assert_eq!(race.phase(), vec![FINISHED, 0.0, -1.0]);
    assert_eq!(race.standings()[FLOATS_PER_STANDING + 7], finish);
}
