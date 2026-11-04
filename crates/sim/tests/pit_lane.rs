//! The pit lane (ADR 0081), through `step`: Bell Deer's lane is driven at the
//! limit and back on the track, the lap counts, and nothing in it can hurt a
//! kart. A stop in a pit box is a later slice; this lane is only driven.

mod common;

use common::{
    follow_centreline, rolling, started, steer_toward, tap, track, with_boxes_at, with_slot_at,
    without_item_boxes, without_wheel_crossings, BELL_DEER, DT, SEED,
};
use sim::tuning::{HAZARD_DROP_DISTANCE_M, PIT_LIMIT_M_PER_S, TYRE_WEAR_ROLLING_PER_S};
use sim::{
    step, BotLevel, Item, ItemSlot, KartInput, KartReadout, RaceEvent, State, Track, TrackError,
    Tyres,
};

/// Metres short of the fork that a staged kart starts from, on the centreline.
const RUN_UP_M: f32 = 25.0;
/// How far ahead along the lane a driver aims.
const LOOK_AHEAD_M: f32 = 8.0;
const HIGHEST_LIMITED_SPEED: f32 = PIT_LIMIT_M_PER_S + 0.001;

fn sidecar_with(edit: impl FnOnce(&str) -> String) -> String {
    edit(&quiet_bell_deer())
}

/// Bell Deer with nothing on it but the lane.
fn quiet_bell_deer() -> String {
    without_item_boxes(&without_wheel_crossings(BELL_DEER))
}

/// The point `metres` along the lane's path from the fork, and the facing
/// a kart there has down it.
fn along_lane(track: &Track, metres: f32) -> ([f32; 3], f32) {
    let lane = track.pit_lane().expect("Bell Deer has a pit lane");
    let at = lane.point_at(metres);
    let ahead = lane.point_at(metres + 1.0);
    (at, f32::atan2(at[0] - ahead[0], at[2] - ahead[2]))
}

/// The sidecar with spawn `slot` on the lane, `metres` along it from the fork.
fn with_slot_on_lane(sidecar: &str, slot: usize, metres: f32) -> String {
    let (position, yaw) = along_lane(&track(sidecar), metres);
    with_slot_at(sidecar, slot, position, yaw)
}

/// The sidecar with spawn slot 0 on the centreline `RUN_UP_M` short of the fork.
fn with_slot_short_of_the_fork(sidecar: &str) -> String {
    let course = track(sidecar);
    let fork = course.pit_lane().expect("a lane").point_at(0.0);
    let s = course.progress_at(fork) - RUN_UP_M;
    let at = course.point_at(s);
    let ahead = course.point_at(s + 1.0);
    with_slot_at(
        sidecar,
        0,
        at,
        f32::atan2(at[0] - ahead[0], at[2] - ahead[2]),
    )
}

/// Full throttle down the lane's path, which starts at the fork: the aim for
/// a kart on the road short of it is the fork itself.
fn drive_lane(state: &State, kart: usize) -> KartInput {
    let track = state.track();
    let lane = track.pit_lane().expect("a lane");
    let here = state.kart_readouts()[kart].position;
    let aim = lane
        .reading(here)
        .map_or(0.0, |metres| metres + LOOK_AHEAD_M);
    let target = lane.point_at(aim);
    steer_toward(state, kart, [target[0], target[2]])
}

/// What the limiter promises of a kart in the lane: its speed and its whole
/// motion along the ground, bump included.
fn assert_at_most_the_limit(kart: &KartReadout) {
    assert!(kart.speed <= HIGHEST_LIMITED_SPEED, "speed {}", kart.speed);
    let along_the_ground = (kart.velocity[0].powi(2) + kart.velocity[2].powi(2)).sqrt();
    assert!(
        along_the_ground <= HIGHEST_LIMITED_SPEED,
        "moving at {along_the_ground}"
    );
}

fn race_on(sidecar: &str, karts: usize, tyres: Tyres) -> State {
    started(State::with_tyres(
        SEED,
        karts,
        0,
        track(sidecar),
        BotLevel::Basic,
        tyres,
    ))
}

#[test]
fn a_kart_in_the_lane_reads_so_only_from_the_entry_line_to_the_exit_line() {
    let mut state = race_on(&sidecar_with(with_slot_short_of_the_fork), 1, Tyres::Off);
    let lane = state.track().pit_lane().expect("a lane").clone();
    let mut seen = Vec::new();
    for _ in 0..1500 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        let Some(metres) = lane.reading(kart.position) else {
            continue;
        };
        seen.push(metres);
        assert_eq!(
            kart.in_pit_lane,
            (lane.entry_m()..=lane.exit_m()).contains(&metres),
            "{metres} m along the lane"
        );
    }
    let furthest = seen.iter().cloned().fold(0.0, f32::max);
    assert!(
        furthest >= lane.exit_m(),
        "it drove the lane, as far as {furthest} m of {} m",
        lane.exit_m()
    );
}

#[test]
fn the_limiter_holds_a_kart_to_the_limit_from_the_line_and_lets_it_go_at_the_exit() {
    let mut state = race_on(&sidecar_with(with_slot_short_of_the_fork), 1, Tyres::Off);
    let mut fastest_before = 0.0_f32;
    let mut fastest_after = 0.0_f32;
    let mut in_lane_ticks = 0;
    for _ in 0..1800 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        if kart.in_pit_lane {
            assert_at_most_the_limit(&kart);
            in_lane_ticks += 1;
        } else if in_lane_ticks > 0 {
            fastest_after = fastest_after.max(kart.speed);
        } else {
            fastest_before = fastest_before.max(kart.speed);
        }
    }
    assert!(fastest_before > PIT_LIMIT_M_PER_S, "free up to the line");
    assert!(in_lane_ticks > 400, "{in_lane_ticks} ticks at the limit");
    assert!(
        fastest_after > 1.5 * PIT_LIMIT_M_PER_S,
        "free past the line, at {fastest_after}"
    );
}

#[test]
fn a_boost_brought_in_ends_at_the_line_and_a_star_gets_no_more_than_the_limit() {
    // A pad just past the entry line: its boost fires in the lane, the most
    // a kart in it could be handed.
    let pad_sidecar = sidecar_with(|sidecar| {
        let course = track(sidecar);
        let lane = course.pit_lane().expect("a lane");
        let (pad, yaw) = along_lane(&course, lane.entry_m() + 15.0);
        let mut edited: serde_json::Value = serde_json::from_str(sidecar).expect("JSON");
        edited["boost_pads"] = serde_json::json!([{ "position": pad, "yaw": yaw }]);
        with_slot_on_lane(&edited.to_string(), 0, lane.entry_m() + 1.0)
    });
    let mut state = race_on(&pad_sidecar, 1, Tyres::Off);
    for tick in 0..240 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        assert!(kart.in_pit_lane, "tick {tick}");
        assert_at_most_the_limit(&kart);
        assert_eq!(kart.boost_s, None, "the boost is off in the lane");
    }

    // A pad short of the line: the boost is burning as the kart comes over it.
    let pad_short_of_the_line = sidecar_with(|sidecar| {
        let course = track(sidecar);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let (pad, yaw) = along_lane(&course, entry - 5.0);
        let mut edited: serde_json::Value = serde_json::from_str(sidecar).expect("JSON");
        edited["boost_pads"] = serde_json::json!([{ "position": pad, "yaw": yaw }]);
        with_slot_on_lane(&edited.to_string(), 0, entry - 12.0)
    });
    let mut state = race_on(&pad_short_of_the_line, 1, Tyres::Off);
    let mut boosted_at_the_line = false;
    let mut was_boosted = false;
    let mut in_lane_ticks = 0;
    for _ in 0..240 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        if kart.in_pit_lane {
            boosted_at_the_line |= was_boosted && in_lane_ticks == 0;
            assert_at_most_the_limit(&kart);
            in_lane_ticks += 1;
        }
        was_boosted = kart.boost_s.is_some();
    }
    assert!(boosted_at_the_line, "the boost reached the line");

    // A star's run fired just short of the line, the kick still burning.
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let short = course.pit_lane().expect("a lane").entry_m() - 8.0;
        let (at, _) = along_lane(&course, short);
        track(&with_slot_on_lane(&with_boxes_at(&quiet, [at]), 2, short))
    };
    let mut state = rolling(build, 3, 0, &[(2, Item::Star)]);
    let idle = KartInput::default();
    state = step(state, &[idle, idle, tap()], DT);
    assert!(state.kart_readouts()[2].star_s.is_some(), "on the run");
    let mut in_lane_ticks = 0;
    for _ in 0..300 {
        let input = drive_lane(&state, 2);
        state = step(state, &[idle, idle, input], DT);
        let kart = state.kart_readouts()[2];
        if kart.in_pit_lane {
            assert_at_most_the_limit(&kart);
            in_lane_ticks += 1;
        }
    }
    assert!(
        in_lane_ticks > 60,
        "{in_lane_ticks} ticks in the lane on the run"
    );
}

#[test]
fn the_limit_beats_a_full_purse() {
    let coin_sidecar = sidecar_with(|sidecar| {
        let course = track(sidecar);
        let coins: Vec<[f32; 3]> = (0..10)
            .map(|k| along_lane(&course, 6.0 + 3.0 * k as f32).0)
            .collect();
        with_slot_on_lane(&common::with_coins_at(sidecar, coins), 0, 2.0)
    });
    let mut state = race_on(&coin_sidecar, 1, Tyres::Off);
    let mut most_coins = 0;
    for _ in 0..600 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        most_coins = most_coins.max(kart.coins);
        if kart.in_pit_lane {
            assert_at_most_the_limit(&kart);
        }
    }
    assert!(most_coins >= 5, "it took {most_coins} coins on the way in");
}

#[test]
fn a_touch_at_the_limit_does_not_carry_a_kart_over_it() {
    // Two karts abreast on the lane, up to the limit, and then one drifts
    // across into the other: the bump goes on top of a kart already at it.
    let across = 2.2;
    let beside = |course: &Track, metres: f32| {
        let (at, yaw) = along_lane(course, metres);
        let (sin, cos) = yaw.sin_cos();
        ([at[0] - cos * across, at[1], at[2] + sin * across], yaw)
    };
    let sidecar = sidecar_with(|sidecar| {
        let course = track(sidecar);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let (abreast, yaw) = beside(&course, entry + 2.0);
        with_slot_on_lane(&with_slot_at(sidecar, 1, abreast, yaw), 0, entry + 2.0)
    });
    let mut state = race_on(&sidecar, 2, Tyres::Off);
    let lane = state.track().pit_lane().expect("a lane").clone();
    let mut touched_at_the_limit = false;
    for tick in 0..300 {
        let line = drive_lane(&state, 0);
        // Holding its own line, then turning in on the kart abreast of it.
        let across_input = if tick < 180 {
            let here = state.kart_readouts()[1].position;
            let metres = lane.reading(here).expect("on the lane") + LOOK_AHEAD_M;
            let (aim, _) = beside(state.track(), metres);
            steer_toward(&state, 1, [aim[0], aim[2]])
        } else {
            drive_lane(&state, 1)
        };
        state = step(state, &[line, across_input], DT);
        for kart in state.kart_readouts() {
            assert!(kart.in_pit_lane, "still in the lane at {:?}", kart.position);
            assert_at_most_the_limit(&kart);
            touched_at_the_limit |= tick >= 180 && kart.bump.is_some();
        }
    }
    assert!(touched_at_the_limit, "the karts met");
}

/// Laps the course from the grid, down the lane at the end of it, until the
/// lap counts. Returns the state and the lowest rise in spline progress over
/// a tick in the lane.
fn lap_through_the_pits(tyres: Tyres) -> (State, f32) {
    let mut state = race_on(&quiet_bell_deer(), 1, tyres);
    let lap = state.track().spline_length();
    let lane = state.track().pit_lane().expect("a lane").clone();
    let fork = state.track().progress_at(lane.point_at(0.0));
    let mut in_lane_ticks = 0_u32;
    let mut lowest_rise = f32::INFINITY;
    let mut last = state.kart_readouts()[0].spline_progress;
    let mut on_the_run_in = false;
    for _ in 0..18_000 {
        let kart = state.kart_readouts()[0];
        if !on_the_run_in && (kart.spline_progress - fork + lap) % lap > lap - RUN_UP_M {
            on_the_run_in = true;
        }
        let input = if on_the_run_in {
            drive_lane(&state, 0)
        } else {
            follow_centreline(&state, 0)
        };
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        if kart.in_pit_lane {
            in_lane_ticks += 1;
            let rise = (kart.spline_progress - last + lap / 2.0).rem_euclid(lap) - lap / 2.0;
            lowest_rise = lowest_rise.min(rise);
        }
        last = kart.spline_progress;
        let counted = state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::LapCompleted { lap: 1, .. }));
        if counted {
            assert!(in_lane_ticks > 100, "{in_lane_ticks} ticks in the lane");
            return (state, lowest_rise);
        }
        assert!(
            !state
                .events()
                .iter()
                .any(|event| matches!(event, RaceEvent::Respawned { .. })),
            "at {:?}",
            kart.position
        );
    }
    panic!("the lap never counted: {:?}", state.kart_readouts()[0]);
}

#[test]
fn a_lap_through_the_pits_counts_with_tyres_off_and_on_and_spline_progress_rises_through_it() {
    for tyres in [Tyres::Off, Tyres::On] {
        let (state, lowest_rise) = lap_through_the_pits(tyres);
        let kart = state.kart_readouts()[0];
        assert_eq!(kart.lap, 2, "{tyres:?}");
        assert!(
            lowest_rise > 0.0,
            "{tyres:?}: progress fell by {lowest_rise} m in a tick in the lane"
        );
    }
}

#[test]
fn the_rest_of_the_lap_counts_on_from_a_lane_past_every_checkpoint_along_it() {
    // A kart on the lane before the start gate takes the start gate and the
    // checkpoint past the building, in order, from inside it.
    let sidecar = sidecar_with(|sidecar| with_slot_on_lane(sidecar, 0, 2.0));
    let mut state = race_on(&sidecar, 1, Tyres::Off);
    assert_eq!(state.kart_readouts()[0].checkpoint, 0);
    let mut crossed = 0;
    for _ in 0..1500 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        crossed = crossed.max(state.kart_readouts()[0].checkpoint);
    }
    assert_eq!(crossed, 1, "the lane's far checkpoint counts");
}

#[test]
fn a_kart_in_the_lane_keeps_its_place_by_spline_progress() {
    // One kart drives the lane past another idling beside it on the straight:
    // it passes it by spline progress, though it is behind the pit wall.
    let sidecar = sidecar_with(|sidecar| {
        let course = track(sidecar);
        let lane = course.pit_lane().expect("a lane");
        let beside = lane.point_at(lane.entry_m() + 40.0);
        let on_the_road = course.point_at(course.progress_at(beside));
        let yaw = along_lane(&course, lane.entry_m() + 40.0).1;
        with_slot_on_lane(
            &with_slot_at(sidecar, 1, on_the_road, yaw),
            0,
            lane.entry_m() + 20.0,
        )
    });
    let mut state = race_on(&sidecar, 2, Tyres::Off);
    assert_eq!(state.kart_readouts()[0].race_position, 2);
    let idle = KartInput::default();
    let mut passed = false;
    for _ in 0..240 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input, idle], DT);
        passed |= state.kart_readouts()[0].race_position == 1;
    }
    assert!(passed);
}

#[test]
fn a_kart_that_leaves_the_lane_respawns_in_it() {
    // Without the lane's outer wall a kart can drive off it onto the grass,
    // where the pit wall stands between it and the centreline.
    let sidecar = sidecar_with(|sidecar| {
        let mut edited: serde_json::Value = serde_json::from_str(sidecar).expect("JSON");
        let meshes = edited["surfaces"].as_array().expect("surfaces").clone();
        edited["surfaces"] = meshes
            .into_iter()
            .filter(|mesh| mesh["name"] != "WallPitLane")
            .collect();
        let edited = edited.to_string();
        let lane_entry = track(&edited).pit_lane().expect("a lane").entry_m();
        with_slot_on_lane(&edited, 0, lane_entry + 30.0)
    });
    let mut state = race_on(&sidecar, 1, Tyres::Off);
    let lane = state.track().pit_lane().expect("a lane").clone();
    let mut respawned = None;
    let hard_right = KartInput {
        steer: 0.6,
        throttle: 1.0,
        ..KartInput::default()
    };
    let mut left = false;
    for _ in 0..1200 {
        // Off the lane's far side, and straight on over the grass.
        let input = if left {
            KartInput {
                throttle: 1.0,
                ..KartInput::default()
            }
        } else {
            hard_right
        };
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        left |= lane.reading(kart.position).is_none();
        if state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::Respawned { kart: 0 }))
        {
            respawned = Some(kart);
            break;
        }
    }
    let kart = respawned.expect("it respawned");
    assert!(
        lane.reading(kart.position).is_some(),
        "back on the lane, at {:?}",
        kart.position
    );
    assert!(kart.in_pit_lane, "and on the limiter");
    assert_eq!(kart.checkpoint, 0, "no gate crossed by it");
}

#[test]
fn a_kart_in_the_lane_cannot_fire_and_keeps_the_item() {
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let (at, _) = along_lane(&course, entry + 10.0);
        track(&with_slot_on_lane(
            &with_boxes_at(&quiet, [at]),
            0,
            entry + 10.0,
        ))
    };
    let mut state = rolling(build, 1, 0, &[(0, Item::Horn)]);
    assert!(state.kart_readouts()[0].in_pit_lane);
    state = step(state, &[tap()], DT);
    assert_eq!(
        state.kart_readouts()[0].item_slot,
        ItemSlot::Held(Item::Horn)
    );
    assert!(state.events().is_empty(), "{:?}", state.events());
}

#[test]
fn a_hazard_held_into_the_lane_does_not_land_there() {
    // Trailed down the road and over the entry line, then let go in the lane.
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let (at, _) = along_lane(&course, entry - 6.0);
        track(&with_slot_on_lane(
            &with_boxes_at(&quiet, [at]),
            2,
            entry - 6.0,
        ))
    };
    let mut state = rolling(build, 3, 0, &[(2, Item::Hazard)]);
    let idle = KartInput::default();
    let press = KartInput {
        use_item: true,
        hold_item: true,
        ..KartInput::default()
    };
    state = step(state, &[idle, idle, press], DT);
    assert_eq!(state.hazards().len(), 1, "trailing");
    let hold = KartInput {
        hold_item: true,
        throttle: 1.0,
        ..KartInput::default()
    };
    for _ in 0..240 {
        let steer = drive_lane(&state, 2).steer;
        state = step(state, &[idle, idle, KartInput { steer, ..hold }], DT);
    }
    assert!(state.kart_readouts()[2].in_pit_lane);
    let lane = state.track().pit_lane().expect("a lane");
    let hazards = state.hazards();
    let [hazard] = hazards.as_slice() else {
        panic!("the hold ended at the line, leaving it there");
    };
    assert!(!hazard.held, "let go at the line");
    let lying_at = lane.reading(hazard.position).expect("on the lane's path");
    assert!(
        lying_at < lane.entry_m(),
        "it lies short of the line, {lying_at} m along of {} m",
        lane.entry_m()
    );
}

#[test]
fn a_hazard_dropped_over_the_line_does_not_land_there() {
    // A kart short of the line, turned to face back up the lane: behind it,
    // where a hazard lands, is past the line.
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let dropper = entry - 0.4;
        let (at, down_the_lane) = along_lane(&course, dropper);
        track(&with_slot_at(
            &with_boxes_at(&quiet, [at]),
            0,
            at,
            down_the_lane + std::f32::consts::PI,
        ))
    };
    let mut state = rolling(build, 1, 0, &[(0, Item::Hazard)]);
    assert!(!state.kart_readouts()[0].in_pit_lane);
    state = step(state, &[tap()], DT);
    assert!(
        state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::ItemFired { kart: 0, .. })),
        "it fired from short of the line"
    );
    assert!(state.hazards().is_empty(), "{:?}", state.hazards());
}

/// Three karts, slot 0 on the lane 40 m past the entry line, and the firing
/// kart in slot 2 standing on the road right beside it with `item` in hand.
fn beside_the_lane(item: Item) -> State {
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let lane = course.pit_lane().expect("a lane");
        let metres = lane.entry_m() + 40.0;
        let (on_lane, yaw) = along_lane(&course, metres);
        // Across the pit wall from the lane's kart: well within any reach.
        let across = 5.5;
        let (sin, cos) = yaw.sin_cos();
        let beside = [on_lane[0] - cos * across, 0.0, on_lane[2] + sin * across];
        let sidecar = with_boxes_at(&quiet, [beside]);
        let sidecar = with_slot_at(&sidecar, 2, beside, yaw);
        track(&with_slot_on_lane(&sidecar, 0, metres))
    };
    rolling(build, 3, 0, &[(2, item)])
}

#[test]
fn a_horn_blown_beside_the_lane_spins_nobody_in_it() {
    let state = beside_the_lane(Item::Horn);
    let idle = KartInput::default();
    assert!(state.kart_readouts()[0].in_pit_lane);
    let state = step(state, &[idle, idle, tap()], DT);
    assert!(
        state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::ItemFired { .. })),
        "it fired"
    );
    assert_eq!(state.kart_readouts()[0].spin_out_s, None);
}

#[test]
fn a_star_kart_spins_out_nobody_in_the_lane() {
    // Fired short of the line, driven down the lane through a kart parked in it.
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let (at, _) = along_lane(&course, entry - 8.0);
        let sidecar = with_slot_on_lane(&with_boxes_at(&quiet, [at]), 2, entry - 8.0);
        track(&with_slot_on_lane(&sidecar, 0, entry + 25.0))
    };
    let mut state = rolling(build, 3, 0, &[(2, Item::Star)]);
    let idle = KartInput::default();
    state = step(state, &[idle, idle, tap()], DT);
    assert!(state.kart_readouts()[2].star_s.is_some(), "on the run");
    for _ in 0..240 {
        let input = drive_lane(&state, 2);
        state = step(state, &[idle, idle, input], DT);
        assert!(
            !state
                .events()
                .iter()
                .any(|event| matches!(event, RaceEvent::SpunOut { .. })),
            "{:?}",
            state.events()
        );
    }
    let (star, parked) = (state.kart_readouts()[2], state.kart_readouts()[0]);
    assert!(star.in_pit_lane, "still in the lane at {:?}", star.position);
    // Along the lane: spline progress wraps at the start gate, which is in it.
    let lane = state.track().pit_lane().expect("a lane");
    assert!(
        lane.reading(star.position) > lane.reading(parked.position),
        "it went through"
    );
}

#[test]
fn a_star_short_of_the_line_spins_out_nobody_past_it() {
    // The star's reach crosses the line before the star does.
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let (at, _) = along_lane(&course, entry - 8.0);
        let sidecar = with_slot_on_lane(&with_boxes_at(&quiet, [at]), 2, entry - 8.0);
        track(&with_slot_on_lane(&sidecar, 0, entry + 0.5))
    };
    let mut state = rolling(build, 3, 0, &[(2, Item::Star)]);
    let idle = KartInput::default();
    assert!(state.kart_readouts()[0].in_pit_lane);
    state = step(state, &[idle, idle, tap()], DT);
    assert!(state.kart_readouts()[2].star_s.is_some(), "on the run");
    let mut reached_the_line = false;
    for _ in 0..180 {
        let input = drive_lane(&state, 2);
        state = step(state, &[idle, idle, input], DT);
        assert_eq!(state.kart_readouts()[0].spin_out_s, None);
        reached_the_line |= state.kart_readouts()[2].in_pit_lane;
    }
    assert!(reached_the_line, "the star came over the line");
}

#[test]
fn a_star_in_the_lane_spins_out_nobody_past_the_exit_until_it_leaves() {
    // A lane short enough for a run to cross, with a kart parked just past
    // its exit line: the star's reach crosses the line before the star does.
    let build = || {
        let quiet = sidecar_with(|sidecar| {
            let course = track(sidecar);
            let exit = course.pit_lane().expect("a lane").entry_m() + 15.0;
            let (at, _) = along_lane(&course, exit);
            let mut edited: serde_json::Value = serde_json::from_str(sidecar).expect("JSON");
            edited["pit_lane"]["exit"]["position"] = serde_json::json!(at);
            edited.to_string()
        });
        let course = track(&quiet);
        let lane = course.pit_lane().expect("a lane");
        let (entry, exit) = (lane.entry_m(), lane.exit_m());
        let (at, _) = along_lane(&course, entry - 8.0);
        let sidecar = with_slot_on_lane(&with_boxes_at(&quiet, [at]), 2, entry - 8.0);
        track(&with_slot_on_lane(&sidecar, 0, exit + 0.5))
    };
    let mut state = rolling(build, 3, 0, &[(2, Item::Star)]);
    let idle = KartInput::default();
    assert!(!state.kart_readouts()[0].in_pit_lane);
    state = step(state, &[idle, idle, tap()], DT);
    assert!(state.kart_readouts()[2].star_s.is_some(), "on the run");
    let mut hit = false;
    for _ in 0..240 {
        let input = drive_lane(&state, 2);
        state = step(state, &[idle, idle, input], DT);
        let (parked, star) = (state.kart_readouts()[0], state.kart_readouts()[2]);
        if parked.spin_out_s.is_some() {
            assert!(!star.in_pit_lane, "spun out from inside the lane");
            hit = true;
            break;
        }
    }
    assert!(hit, "past the line the star is a star again");
}

#[test]
fn a_homing_projectile_gives_up_on_a_kart_in_the_lane() {
    // Fired from the road short of the fork at the kart ahead, in the lane.
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let lane = course.pit_lane().expect("a lane");
        let road = course.point_at(course.progress_at(lane.point_at(0.0)) - 10.0);
        let ahead = course.point_at(course.progress_at(lane.point_at(0.0)) - 9.0);
        let yaw = f32::atan2(road[0] - ahead[0], road[2] - ahead[2]);
        let sidecar = with_slot_at(&with_boxes_at(&quiet, [road]), 1, road, yaw);
        track(&with_slot_on_lane(&sidecar, 0, lane.entry_m() + 30.0))
    };
    let mut state = rolling(build, 2, 0, &[(1, Item::Homing)]);
    let idle = KartInput::default();
    assert_eq!(state.kart_readouts()[1].race_position, 2);
    state = step(state, &[idle, tap()], DT);
    assert!(state
        .events()
        .iter()
        .any(|event| matches!(event, RaceEvent::ItemFired { kart: 1, .. })));
    state = step(state, &[idle, idle], DT);
    assert!(state.projectiles().is_empty(), "it gave up");
    for _ in 0..180 {
        state = step(state, &[idle, idle], DT);
    }
    assert_eq!(state.kart_readouts()[0].spin_out_s, None);
}

#[test]
fn tyres_wear_only_by_rolling_in_the_lane() {
    // Weaving hard and braking in the lane, at the limit: what would wear a
    // kart anywhere else costs nothing here but the roll.
    let sidecar = sidecar_with(|sidecar| {
        let lane_entry = track(sidecar).pit_lane().expect("a lane").entry_m();
        with_slot_on_lane(sidecar, 0, lane_entry + 1.0)
    });
    let mut state = race_on(&sidecar, 1, Tyres::On);
    // Up to the limit first, in a line, so the wear measured is the moving kind.
    for _ in 0..120 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
    }
    let before = state.kart_readouts()[0].tyre_life;
    let ticks = 120;
    for tick in 0..ticks {
        let mut input = drive_lane(&state, 0);
        // Into the steering by turns, and a touch of brake on each.
        input.steer = (input.steer + if tick % 40 < 20 { 0.7 } else { -0.7 }).clamp(-1.0, 1.0);
        input.brake = 0.2;
        state = step(state, &[input], DT);
        assert!(state.kart_readouts()[0].in_pit_lane);
    }
    let lost = before - state.kart_readouts()[0].tyre_life;
    let rolling_only = TYRE_WEAR_ROLLING_PER_S * ticks as f32 * DT;
    assert!(
        (lost - rolling_only).abs() < 1e-3,
        "lost {lost}, rolling alone costs {rolling_only}"
    );
}

#[test]
fn a_hazard_lying_short_of_the_line_does_not_reach_a_kart_past_it() {
    // A kart turned to face back up the lane drops its hazard toward the line,
    // inside the reach of a kart parked just over it.
    let build = || {
        let quiet = quiet_bell_deer();
        let course = track(&quiet);
        let entry = course.pit_lane().expect("a lane").entry_m();
        let dropper = entry - 0.4 - HAZARD_DROP_DISTANCE_M;
        let (at, down_the_lane) = along_lane(&course, dropper);
        let sidecar = with_slot_at(
            &with_boxes_at(&quiet, [at]),
            2,
            at,
            down_the_lane + std::f32::consts::PI,
        );
        track(&with_slot_on_lane(&sidecar, 0, entry + 0.5))
    };
    let mut state = rolling(build, 3, 0, &[(2, Item::Hazard)]);
    let idle = KartInput::default();
    assert!(state.kart_readouts()[0].in_pit_lane);
    state = step(state, &[idle, idle, tap()], DT);
    for _ in 0..30 {
        state = step(state, &[idle, idle, idle], DT);
        assert_eq!(state.kart_readouts()[0].spin_out_s, None);
    }
    assert_eq!(state.hazards().len(), 1, "it still lies there, unused");
}

#[test]
fn a_touch_in_the_lane_costs_no_tyre_life() {
    // Into a kart parked in the lane: the bump is real, the wear is the roll's.
    let sidecar = sidecar_with(|sidecar| {
        let entry = track(sidecar).pit_lane().expect("a lane").entry_m();
        with_slot_on_lane(&with_slot_on_lane(sidecar, 0, entry + 1.0), 1, entry + 12.0)
    });
    let mut state = race_on(&sidecar, 2, Tyres::On);
    let idle = KartInput::default();
    let mut touched = false;
    let ticks = 180;
    for _ in 0..ticks {
        let input = drive_lane(&state, 0);
        state = step(state, &[input, idle], DT);
        touched |= state.kart_readouts()[0].bump.is_some();
    }
    assert!(touched, "the karts met");
    let lost = 100.0 - state.kart_readouts()[0].tyre_life;
    let rolling_at_most = TYRE_WEAR_ROLLING_PER_S * ticks as f32 * DT;
    assert!(
        lost <= rolling_at_most + 1e-3,
        "lost {lost}, rolling costs at most {rolling_at_most}"
    );
}

#[test]
fn a_sidecar_with_a_malformed_lane_is_rejected() {
    let with_lane = |edit: fn(&mut serde_json::Value)| {
        let mut sidecar: serde_json::Value = serde_json::from_str(BELL_DEER).expect("JSON");
        edit(&mut sidecar["pit_lane"]);
        Track::from_sidecar(&sidecar.to_string())
    };
    assert!(with_lane(|_| {}).is_ok());
    for (reason, edit) in [
        (
            "seven boxes",
            (|lane| {
                lane["pit_boxes"].as_array_mut().expect("pit boxes").pop();
            }) as fn(&mut serde_json::Value),
        ),
        ("a line off the path", |lane| {
            lane["entry"]["position"] = serde_json::json!([500.0, 0.0, 500.0]);
        }),
        ("lines the wrong way round", |lane| {
            let entry = lane["entry"].clone();
            lane["entry"] = lane["exit"].clone();
            lane["exit"] = entry;
        }),
    ] {
        assert!(
            matches!(with_lane(edit), Err(TrackError::Invalid(_))),
            "{reason}"
        );
    }
}

#[test]
fn the_oval_has_no_lane() {
    assert!(common::oval().pit_lane().is_none());
}
