//! The determinism gate: a state hash over everything that influences future
//! ticks, and replays `{seed, track, inputs[]}` that rerun a race and report
//! that hash at every 60th tick. The committed fixtures pin the hashes the
//! sim produced when they were recorded; the web app's tests run the same
//! fixtures through the wasm build.

mod common;

use common::{
    landed, oval, started, throttle, ticks, track, with_boxes_under, without_item_boxes, DT, OVAL,
};

use sim::tuning::{KART_HALF_LENGTH_M, REPLAY_HASH_INTERVAL_TICKS};
use sim::{
    step, BotLevel, KartInput, MiniTurbo, Phase, RaceEvent, Replay, Segment, State, Surface, Track,
    Tyres,
};

fn race(seed: u64, kart_count: usize) -> State {
    State::new(seed, kart_count, oval())
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
    let still = step(started(race(7, 2)), &[KartInput::default(); 2], DT);
    let second_drives = step(started(race(7, 2)), &[KartInput::default(), THROTTLE], DT);
    assert_ne!(still.hash(), second_drives.hash());
}

#[test]
fn the_phase_its_clocks_and_a_karts_start_are_part_of_the_hash() {
    let idle = [KartInput::default()];
    let one_tick_in = step(race(7, 1), &idle, DT);
    let two_ticks_in = step(step(race(7, 1), &idle, DT), &idle, DT);
    assert_ne!(
        one_tick_in.hash(),
        two_ticks_in.hash(),
        "the countdown's clock, since nothing else moves"
    );
    assert_ne!(race(7, 1).hash(), started(race(7, 1)).hash(), "the phase");
    let pressed_early = step(race(7, 1), &[THROTTLE], DT);
    assert_ne!(
        one_tick_in.hash(),
        pressed_early.hash(),
        "a stall set up during the countdown"
    );
}

#[test]
fn speed_position_and_heading_are_part_of_the_hash() {
    let idle = step(started(race(7, 1)), &[KartInput::default()], DT);
    let driving = step(started(race(7, 1)), &[THROTTLE], DT);
    let turning = step(started(race(7, 1)), &[TURN], DT);
    assert_ne!(idle.hash(), driving.hash(), "speed and position");
    assert_ne!(driving.hash(), turning.hash(), "heading");
}

/// The oval with one item box, under spawn slot 0, picked up at Go.
fn boxed_oval() -> Track {
    track(&with_boxes_under(&without_item_boxes(OVAL), [0]))
}

/// A one-kart race on the boxed oval the tick the item the box rolled at Go
/// from `seed` lands in the kart's slot.
fn holding(seed: u64) -> (State, sim::Item) {
    let state = step(
        started(State::new(seed, 1, boxed_oval())),
        &[KartInput::default()],
        DT,
    );
    let [sim::RaceEvent::ItemPicked {
        item: Some(item), ..
    }] = state.events()
    else {
        panic!("one pickup: {:?}", state.events());
    };
    let item = *item;
    (landed(state, 1, [0]), item)
}

#[test]
fn a_karts_item_is_part_of_the_hash() {
    // The seeds are tried until the roll is a homing projectile, which a
    // lone kart fires to no effect, having nobody ahead: the two states
    // then differ in the kart's slot and nothing else.
    let seed = (0..)
        .find(|&seed| holding(seed).1 == sim::Item::Homing)
        .expect("some seed rolls a homing projectile");
    let idle = step(holding(seed).0, &[KartInput::default()], DT);
    let fired = step(
        holding(seed).0,
        &[KartInput {
            use_item: true,
            ..KartInput::default()
        }],
        DT,
    );
    assert_ne!(idle.hash(), fired.hash(), "the slot emptied");
}

#[test]
fn a_hazards_place_is_part_of_the_hash() {
    // Two karts fire the same hazard and drive the same way, one letting
    // go of the button a tick before the other: the karts end up alike and
    // only the hazards lie in different places.
    let seed = (0..)
        .find(|&seed| holding(seed).1 == sim::Item::Hazard)
        .expect("some seed rolls a hazard");
    let hold_until = |release_tick: u32| {
        let mut state = holding(seed).0;
        for tick in 0..30 {
            state = step(
                state,
                &[KartInput {
                    throttle: 1.0,
                    hold_item: tick < release_tick,
                    ..KartInput::default()
                }],
                DT,
            );
        }
        state
    };
    let (early, late) = (hold_until(10), hold_until(11));
    assert_eq!(
        early.kart_readouts(),
        late.kart_readouts(),
        "the karts drove alike"
    );
    assert_ne!(early.hazards(), late.hazards());
    assert_ne!(early.hash(), late.hash(), "the hazard moved");
}

#[test]
fn a_projectiles_place_is_part_of_the_hash() {
    // Two karts stand on the grid, the one on the second slot with a
    // projectile; it fires a tick earlier in one race than the other and
    // both then stand still, so only the projectiles lie in different
    // places. Three ticks, short of the projectile reaching the pole kart.
    let boxed = || track(&with_boxes_under(&without_item_boxes(OVAL), [1]));
    let rolls_a_projectile = |seed: u64| {
        let state = step(
            started(State::new(seed, 2, boxed())),
            &[KartInput::default(); 2],
            DT,
        );
        state.events().iter().any(|event| {
            matches!(
                event,
                sim::RaceEvent::ItemPicked {
                    kart: 1,
                    item: Some(sim::Item::Homing),
                    ..
                }
            )
        })
    };
    let seed = (0..)
        .find(|&seed| rolls_a_projectile(seed))
        .expect("some seed rolls a projectile");
    let fire_at = |fire_tick: u32| {
        let mut state = landed(
            step(
                started(State::new(seed, 2, boxed())),
                &[KartInput::default(); 2],
                DT,
            ),
            2,
            [1],
        );
        for tick in 0..3 {
            let fire = KartInput {
                use_item: tick == fire_tick,
                ..KartInput::default()
            };
            state = step(state, &[KartInput::default(), fire], DT);
        }
        state
    };
    let (early, late) = (fire_at(0), fire_at(1));
    assert_eq!(
        early.kart_readouts(),
        late.kart_readouts(),
        "the karts stood alike"
    );
    assert_eq!(early.projectiles().len(), 1);
    assert_ne!(early.projectiles(), late.projectiles());
    assert_ne!(early.hash(), late.hash(), "the projectile moved");
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
    assert_eq!(replay.human_count(), 1);
    assert_eq!(replay.tick_count(), 150);
    assert_eq!(REPLAY_HASH_INTERVAL_TICKS, 60);

    let hashes = replay.run(oval()).expect("the replay runs on the oval");

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
        bots: 0,
        level: BotLevel::Basic,
        tyres: Tyres::Off,
        laps: 3,
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
    let error = replay_of("bell-deer", vec![held(60, 1)])
        .run(oval())
        .unwrap_err();
    assert!(error.to_string().contains("bell-deer"), "{error}");
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
/// by a wall (the track colliders), every track, a drift released into orange
/// and another into red (the cornering rules), and a rocket start
/// boosting at Go and another stalling there (the start rule), a coin
/// taken and coins lost to a spin-out (the coin rules). A fixture
/// that only drives up a straight would pass on a sim with most of its
/// rules broken.
#[test]
fn the_fixture_set_exercises_kart_contact_off_track_walls_every_track_both_mini_turbos_a_rocket_start_a_stall_the_kerb_the_wheel_coins_a_stop_and_ace_bots(
) {
    let mut tracks = std::collections::BTreeSet::new();
    let mut levels = std::collections::BTreeSet::new();
    let mut tyres = std::collections::BTreeSet::new();
    let mut surfaces = std::collections::HashSet::new();
    let mut closest_karts = f32::MAX;
    let mut held_by_a_wall = false;
    let mut released_orange = false;
    let mut released_red = false;
    let mut rocket_started = false;
    let mut stalled = false;
    let mut launched_by_a_kerb = false;
    let mut hit_by_the_wheel = false;
    let mut took_a_coin = false;
    let mut lost_coins = false;
    let mut towed = false;
    let mut lapped_in_the_pits = false;
    let mut stopped_in_a_pit_box = false;
    for (_, replay) in fixtures() {
        tracks.insert(replay.track.clone());
        levels.insert(replay.level.name());
        tyres.insert(replay.tyres.name());
        // The race the fixture records: the same inputs on another pick drive elsewhere.
        let mut state = State::with_tyres(
            replay.seed,
            replay.human_count(),
            replay.bots,
            sidecar(&replay.track),
            replay.level,
            replay.tyres,
        )
        .with_laps(replay.laps);
        for segment in &replay.segments {
            for _ in 0..segment.ticks {
                let before = state.kart_readouts();
                let counting_down = matches!(state.phase(), Phase::Countdown { .. });
                state = step(state, &segment.inputs, DT);
                let go = counting_down && matches!(state.phase(), Phase::Racing { .. });
                hit_by_the_wheel |= state
                    .events()
                    .iter()
                    .any(|event| matches!(event, RaceEvent::SpunOut { by: None, .. }));
                for event in state.events() {
                    if let RaceEvent::LapCompleted { kart, .. } = event {
                        lapped_in_the_pits |= state.kart_readouts()[*kart].in_pit_lane;
                    }
                    took_a_coin |= matches!(event, RaceEvent::CoinPicked { .. });
                    lost_coins |= matches!(event, RaceEvent::CoinsLost { .. });
                }
                let transforms = state.kart_readouts();
                for (slot, kart) in transforms.iter().enumerate() {
                    surfaces.insert(state.kart_surface(slot));
                    towed |= matches!(kart.slipstream, Some(sim::Slipstream::Tow { .. }));
                    stopped_in_a_pit_box |= kart.stop.is_some_and(|stop| stop.on_the_mark());
                    // A drift that ends with a boost was let go; the tier
                    // it paid is the one it showed the tick before, since
                    // the release tick adds no charge.
                    if let (Some(drift), None, Some(_)) =
                        (before[slot].drift, kart.drift, kart.boost_s)
                    {
                        released_orange |= drift.mini_turbo() == MiniTurbo::Orange;
                        released_red |= drift.mini_turbo() == MiniTurbo::Red;
                    }
                    // Off the ground over the kerb without a hop pressed.
                    launched_by_a_kerb |= !before[slot].airborne
                        && kart.airborne
                        && !segment.inputs.get(slot).is_some_and(|input| input.hop)
                        && state.track().on_a_kerb(before[slot].position);
                    rocket_started |= go && kart.boost_s.is_some();
                    stalled |= go && kart.stall_s.is_some();
                    // Only a wall takes most of a lone kart's speed in one
                    // tick while the throttle is held and the kart stays
                    // put: the brake takes more than a second over it, and
                    // a respawn moves the kart to a checkpoint.
                    let throttled = segment
                        .inputs
                        .get(slot)
                        .is_some_and(|input| input.throttle > 0.5);
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
    assert_eq!(
        tracks,
        ["bell-deer", "goldstone", "monarch", "oval"]
            .map(String::from)
            .into(),
        "every track"
    );
    assert!(
        levels.contains("ace"),
        "no fixture races Ace bots, so a retune of Ace could pass the gate: {levels:?}"
    );
    assert!(
        tyres.contains("on"),
        "no fixture wears its tyres, so a retune of wear could pass the gate: {tyres:?}"
    );
    for surface in [Surface::Asphalt, Surface::Sand] {
        assert!(
            surfaces.contains(&surface),
            "no fixture reaches {surface:?}"
        );
    }
    assert!(held_by_a_wall, "no lone kart is held by a wall");
    assert!(released_orange, "no fixture releases an orange mini-turbo");
    assert!(released_red, "no fixture releases a red mini-turbo");
    assert!(rocket_started, "no fixture boosts at Go");
    assert!(stalled, "no fixture stalls at Go");
    assert!(launched_by_a_kerb, "no fixture is launched by a kerb");
    assert!(hit_by_the_wheel, "no fixture is hit by the runaway wheel");
    assert!(took_a_coin, "no fixture takes a coin");
    assert!(lost_coins, "no fixture loses coins to a spin-out");
    assert!(towed, "no fixture tows a kart");
    assert!(
        lapped_in_the_pits,
        "no fixture completes a lap in the pit lane"
    );
    assert!(stopped_in_a_pit_box, "no fixture stops in a pit box");
    // Centres within a kart's length of each other are in contact or about
    // to be, whichever way the two karts face.
    let touching = 2.0 * KART_HALF_LENGTH_M + 0.05;
    assert!(
        closest_karts < touching,
        "no two karts touch: closest {closest_karts:.2} m"
    );
}

fn distance(a: &sim::KartReadout, b: &sim::KartReadout) -> f32 {
    (0..3)
        .map(|axis| (a.position[axis] - b.position[axis]).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[test]
fn a_replay_with_bots_reruns_them_from_the_seed_and_carries_their_count() {
    let bots = 2;
    let json = Replay {
        seed: 7,
        track: "oval".to_owned(),
        bots,
        level: BotLevel::Basic,
        tyres: Tyres::Off,
        laps: 3,
        segments: vec![held(120, 1)],
        hashes: Vec::new(),
    }
    .to_json();
    assert!(json.contains("\"bots\": 2"), "{json}");
    let replay = Replay::from_json(&json).unwrap();
    assert_eq!(replay.bots, bots);
    assert_eq!(
        replay.human_count(),
        1,
        "the segments hold the humans' inputs"
    );

    let hashes = replay.run(oval()).expect("the replay runs with its bots");
    let mut state = State::with_bots(7, 1, bots, oval(), BotLevel::Basic);
    let mut expected = Vec::new();
    for tick in 1..=120 {
        state = step(state, &[THROTTLE], DT);
        if tick % 60 == 0 {
            expected.push(state.hash());
        }
    }
    assert_eq!(hashes, expected);
    assert_eq!(state.kart_readouts().len(), 3);

    // Bots count against the grid too.
    let error = Replay {
        bots: 8,
        ..replay.clone()
    }
    .run(oval())
    .unwrap_err();
    assert!(error.to_string().contains("9 karts"), "{error}");
}

#[test]
fn a_replay_recorded_before_bots_existed_has_none() {
    assert_eq!(Replay::from_json(REPLAY).unwrap().bots, 0);
}

#[test]
fn a_replay_recorded_before_bot_levels_existed_runs_its_bots_at_basic() {
    assert_eq!(Replay::from_json(REPLAY).unwrap().level, BotLevel::Basic);
}

/// A lone bot's replay on the oval, long enough to take it past its fifth lap,
/// with `laps` as the JSON's field when there is one.
fn lone_bot_replay(laps: Option<u32>) -> Replay {
    let field = laps.map_or(String::new(), |name| format!(r#""laps": {name},"#));
    Replay::from_json(&format!(
        r#"{{ "version": 1, "seed": "7", "track": "oval", "bots": 1, {field}
             "segments": [{{ "ticks": {}, "inputs": [] }}], "hashes": [] }}"#,
        ticks(240.0)
    ))
    .expect("the replay parses")
}

#[test]
fn a_replay_without_laps_runs_three() {
    let replay = lone_bot_replay(None);
    assert_eq!(replay.laps, 3);
    let hashes = replay.run(sidecar("oval")).unwrap();

    // The same race by hand, every hash held to it, finishing on its third lap.
    let mut state = State::with_bots(7, 0, 1, sidecar("oval"), BotLevel::Basic);
    let mut by_hand = Vec::new();
    for tick in 1..=ticks(240.0) {
        state = step(state, &[], DT);
        if (tick as u32).is_multiple_of(REPLAY_HASH_INTERVAL_TICKS) {
            by_hand.push(state.hash());
        }
    }
    assert_eq!(hashes, by_hand);
    assert_eq!(state.standings()[0].laps_completed, 3);
}

#[test]
fn a_replay_carries_its_laps_through_the_json_and_reruns_it() {
    let replay = lone_bot_replay(Some(5));
    assert_eq!(replay.laps, 5);
    assert_eq!(Replay::from_json(&replay.to_json()).unwrap(), replay);
    assert_ne!(
        replay.run(sidecar("oval")).unwrap(),
        lone_bot_replay(None).run(sidecar("oval")).unwrap(),
        "the bot keeps lapping past a third lap"
    );
}

#[test]
fn a_replay_of_laps_nobody_can_pick_is_refused() {
    let json = lone_bot_replay(Some(5))
        .to_json()
        .replace("\"laps\": 5", "\"laps\": 4");
    assert!(Replay::from_json(&json).is_err());
}

#[test]
fn a_replay_recorded_before_the_tyres_pick_races_off() {
    assert_eq!(Replay::from_json(REPLAY).unwrap().tyres, Tyres::Off);
}

#[test]
fn a_replay_carries_its_tyres_pick_and_reruns_with_it() {
    let at = |tyres: Tyres| Replay {
        seed: 7,
        track: "oval".to_owned(),
        bots: 0,
        level: BotLevel::Basic,
        tyres,
        laps: 3,
        segments: vec![Segment {
            ticks: 600,
            inputs: vec![throttle()],
        }],
        hashes: Vec::new(),
    };
    let json = at(Tyres::On).to_json();
    assert!(json.contains("\"tyres\": \"on\""), "{json}");
    let replay = Replay::from_json(&json).unwrap();
    assert_eq!(replay.tyres, Tyres::On);

    assert_ne!(
        replay.run(oval()).unwrap(),
        at(Tyres::Off).run(oval()).unwrap(),
        "tyre life enters the hash"
    );
}

#[test]
fn a_replay_carries_its_bot_level_and_reruns_its_bots_at_it() {
    let json = Replay {
        seed: 7,
        track: "oval".to_owned(),
        bots: 1,
        level: BotLevel::Ace,
        tyres: Tyres::Off,
        laps: 3,
        // Ten seconds: well past Go, where the levels' bots part.
        segments: vec![held(600, 1)],
        hashes: Vec::new(),
    }
    .to_json();
    assert!(json.contains("\"level\": \"ace\""), "{json}");
    let replay = Replay::from_json(&json).unwrap();
    assert_eq!(replay.level, BotLevel::Ace);

    let hashes = replay.run(oval()).expect("the replay runs at its level");
    let mut ace = State::with_bots(7, 1, 1, oval(), BotLevel::Ace);
    let mut basic = State::with_bots(7, 1, 1, oval(), BotLevel::Basic);
    let mut expected = Vec::new();
    for tick in 1..=600 {
        ace = step(ace, &[THROTTLE], DT);
        basic = step(basic, &[THROTTLE], DT);
        if tick % REPLAY_HASH_INTERVAL_TICKS == 0 {
            expected.push(ace.hash());
        }
    }
    assert_eq!(hashes, expected);
    assert_ne!(
        ace.hash(),
        basic.hash(),
        "the level changes how the bot drives"
    );
}
