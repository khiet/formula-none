//! The determinism gate: a state hash over everything that influences future
//! ticks, and replays `{seed, track, inputs[]}` that rerun a race and report
//! that hash at every 60th tick. The committed fixtures pin the hashes the
//! sim produced when they were recorded; the web app's tests run the same
//! fixtures through the wasm build.

use sim::tuning::{
    KART_HALF_LENGTH_M, REPLAY_HASH_INTERVAL_TICKS, TICKS_PER_SECOND, TOP_SPEED_M_PER_S,
};
use sim::{step, KartInput, Replay, Segment, State, Surface, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;

fn race(seed: u64, kart_count: usize) -> State {
    State::new(
        seed,
        kart_count,
        Track::from_sidecar(OVAL).expect("the oval fixture loads"),
    )
}

const THROTTLE: KartInput = KartInput {
    steer: 0.0,
    throttle: 1.0,
    brake: 0.0,
    hop: false,
    use_item: false,
    hold_item: false,
};
const TURN: KartInput = KartInput {
    steer: 1.0,
    ..THROTTLE
};

#[test]
fn equal_races_hash_equal() {
    assert_eq!(race(7, 2).hash(), race(7, 2).hash());
}

#[test]
fn the_seed_is_part_of_the_hash() {
    assert_ne!(race(7, 1).hash(), race(8, 1).hash());
}

#[test]
fn every_kart_is_part_of_the_hash() {
    assert_ne!(race(7, 1).hash(), race(7, 2).hash());
    let still = step(race(7, 2), &[KartInput::default(); 2], DT);
    let second_drives = step(race(7, 2), &[KartInput::default(), THROTTLE], DT);
    assert_ne!(still.hash(), second_drives.hash());
}

#[test]
fn speed_position_and_heading_are_part_of_the_hash() {
    let idle = step(race(7, 1), &[KartInput::default()], DT);
    let driving = step(race(7, 1), &[THROTTLE], DT);
    let turning = step(race(7, 1), &[TURN], DT);
    assert_ne!(idle.hash(), driving.hash(), "speed and position");
    assert_ne!(driving.hash(), turning.hash(), "heading");
}

const REPLAY: &str = r#"{
  "version": 1,
  "seed": "7",
  "track": "oval",
  "segments": [
    { "ticks": 90, "inputs": [{ "steer": 0.0, "throttle": 1.0, "brake": 0.0 }] },
    { "ticks": 60, "inputs": [{ "steer": 1.0, "throttle": 1.0, "brake": 0.0 }] }
  ],
  "hashes": []
}"#;

#[test]
fn a_replay_reruns_its_inputs_and_hashes_the_state_at_every_60th_tick() {
    let replay = Replay::from_json(REPLAY).expect("the replay parses");
    assert_eq!(replay.kart_count(), 1);
    assert_eq!(replay.tick_count(), 150);
    assert_eq!(REPLAY_HASH_INTERVAL_TICKS, 60);

    let hashes = replay
        .run(Track::from_sidecar(OVAL).unwrap())
        .expect("the replay runs on the oval");

    let mut state = race(7, 1);
    let mut expected = Vec::new();
    for tick in 1..=150 {
        state = step(state, &[if tick <= 90 { THROTTLE } else { TURN }], DT);
        if tick % 60 == 0 {
            expected.push(state.hash());
        }
    }
    assert_eq!(hashes, expected);
    assert_eq!(
        hashes.len(),
        2,
        "ticks 60 and 120; the last 30 ticks are not hashed"
    );
}

#[test]
fn a_replay_recorded_before_the_buttons_existed_reads_them_as_unpressed() {
    let replay = Replay::from_json(REPLAY).unwrap();
    assert_eq!(replay.segments[0].inputs, vec![THROTTLE]);

    let pressed = KartInput {
        hop: true,
        use_item: true,
        hold_item: true,
        ..THROTTLE
    };
    let json = replay_of(
        "oval",
        vec![Segment {
            ticks: 1,
            inputs: vec![pressed],
        }],
    )
    .to_json();
    assert!(json.contains("\"useItem\": true"), "{json}");
    assert_eq!(
        Replay::from_json(&json).unwrap().segments[0].inputs,
        vec![pressed]
    );
}

#[test]
fn a_replay_round_trips_through_json_with_its_hashes_as_hex() {
    let mut replay = Replay::from_json(REPLAY).unwrap();
    replay.hashes = vec![0x3e0a_d4e8_4a0b_22c7, 1];
    let json = replay.to_json();
    assert!(json.contains("\"3e0ad4e84a0b22c7\""), "{json}");
    assert!(json.contains("\"0000000000000001\""), "{json}");
    let again = Replay::from_json(&json).unwrap();
    assert_eq!(again.hashes, replay.hashes);
    assert_eq!(again.to_json(), json);
}

fn replay_of(track: &str, segments: Vec<Segment>) -> Replay {
    Replay {
        seed: 7,
        track: track.to_owned(),
        segments,
        hashes: Vec::new(),
    }
}

fn held(ticks: u32, karts: usize) -> Segment {
    Segment {
        ticks,
        inputs: vec![THROTTLE; karts],
    }
}

#[test]
fn a_replay_rejects_the_wrong_track_too_many_karts_or_an_uneven_grid() {
    let oval = || Track::from_sidecar(OVAL).unwrap();

    let error = replay_of("brands-patch", vec![held(60, 1)])
        .run(oval())
        .unwrap_err();
    assert!(error.to_string().contains("brands-patch"), "{error}");
    assert!(error.to_string().contains("oval"), "{error}");

    let error = replay_of("oval", vec![held(60, 9)])
        .run(oval())
        .unwrap_err();
    assert!(error.to_string().contains("8 spawn slots"), "{error}");

    let error = replay_of("oval", vec![held(60, 2), held(60, 1)])
        .run(oval())
        .unwrap_err();
    assert!(error.to_string().contains("segment 1"), "{error}");

    assert!(Replay::from_json("{}").is_err());
    assert!(Replay::from_json(&REPLAY.replace("\"version\": 1", "\"version\": 2")).is_err());
}

/// The committed fixtures under `assets/replays`, each with the hashes it
/// reported when recorded. A change to the rules changes them all;
/// `cargo run -p sim --example replay -- --update` re-records them.
fn fixtures() -> Vec<(String, Replay)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/replays");
    let mut fixtures: Vec<_> = std::fs::read_dir(dir)
        .expect("assets/replays exists")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(".replay.json"))
        .map(|path| {
            let json = std::fs::read_to_string(&path).unwrap();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let replay = Replay::from_json(&json).unwrap_or_else(|error| panic!("{name}: {error}"));
            (name, replay)
        })
        .collect();
    fixtures.sort_by(|a, b| a.0.cmp(&b.0));
    fixtures
}

fn sidecar(track: &str) -> Track {
    let path = format!(
        "{}/../../assets/tracks/{track}/{track}.track.json",
        env!("CARGO_MANIFEST_DIR")
    );
    Track::from_sidecar(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn every_committed_fixture_replays_to_the_hashes_it_was_recorded_with() {
    let fixtures = fixtures();
    assert!(fixtures.len() >= 3, "at least three regression fixtures");
    for (name, replay) in fixtures {
        assert!(!replay.hashes.is_empty(), "{name} was recorded with hashes");
        let hashes = replay
            .run(sidecar(&replay.track))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(hashes, replay.hashes, "{name} diverged from its recording");
    }
}

/// What the fixture set must exercise for the gate to mean anything: karts
/// touching (the contact solver), karts off the asphalt and a lone kart held
/// by a wall (the track colliders), both tracks, and a drift released into a
/// mini-turbo (the cornering rules). A fixture that only drives up a
/// straight would pass on a sim with most of its rules broken.
#[test]
fn the_fixture_set_exercises_kart_contact_off_track_walls_both_tracks_and_a_mini_turbo() {
    let mut tracks = std::collections::BTreeSet::new();
    let mut surfaces = std::collections::HashSet::new();
    let mut closest_karts = f32::MAX;
    let mut held_by_a_wall = false;
    let mut drifted = false;
    let mut boosted = false;
    for (_, replay) in fixtures() {
        tracks.insert(replay.track.clone());
        let mut state = State::new(replay.seed, replay.kart_count(), sidecar(&replay.track));
        for segment in &replay.segments {
            for _ in 0..segment.ticks {
                let before = state.kart_transforms();
                state = step(state, &segment.inputs, DT);
                let transforms = state.kart_transforms();
                for (slot, kart) in transforms.iter().enumerate() {
                    surfaces.insert(state.kart_surface(slot));
                    drifted |= kart.drift.is_some();
                    boosted |= kart.speed > TOP_SPEED_M_PER_S;
                    // Only a wall takes most of a lone kart's speed in one
                    // tick while the throttle is held and the kart stays
                    // put: the brake takes more than a second over it, and
                    // a respawn moves the kart to a checkpoint.
                    let throttled = segment.inputs[slot].throttle > 0.5;
                    if transforms.len() == 1
                        && throttled
                        && before[slot].speed > 5.0
                        && kart.speed < 0.25 * before[slot].speed
                        && distance(&before[slot], kart) < 1.0
                    {
                        held_by_a_wall = true;
                    }
                    for other in &transforms[slot + 1..] {
                        closest_karts = closest_karts.min(distance(kart, other));
                    }
                }
            }
        }
    }
    assert_eq!(tracks.len(), 2, "both tracks: {tracks:?}");
    for surface in [Surface::Asphalt, Surface::Sand, Surface::Grass] {
        assert!(
            surfaces.contains(&surface),
            "no fixture reaches {surface:?}"
        );
    }
    assert!(held_by_a_wall, "no lone kart is held by a wall");
    assert!(drifted, "no fixture drifts");
    assert!(boosted, "no fixture releases a mini-turbo");
    // Centres within a kart's length of each other are in contact or about
    // to be, whichever way the two karts face.
    let touching = 2.0 * KART_HALF_LENGTH_M + 0.05;
    assert!(
        closest_karts < touching,
        "no two karts touch: closest {closest_karts:.2} m"
    );
}

fn distance(a: &sim::KartTransform, b: &sim::KartTransform) -> f32 {
    (0..3)
        .map(|axis| (a.position[axis] - b.position[axis]).powi(2))
        .sum::<f32>()
        .sqrt()
}
