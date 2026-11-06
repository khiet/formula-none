//! The stop in a pit box (ADR 0082), through `step` and `stop_roll`: a kart
//! on Tyres On that reaches its pit box is taken onto its mark, held there
//! for the time the seed rolls, dead to its controls and out of everyone's
//! way, and let go on green on new tyres; on Tyres Off the pit boxes do nothing.

mod common;

use std::collections::BTreeMap;

use common::{
    along_lane, drive_lane, quiet_bell_deer, steer_toward, track, with_boxes_at, with_pit_box_at,
    with_slot_on_lane, DT, SEED,
};
use sim::tuning::{
    KART_HALF_LENGTH_M, PIT_BOX_CATCH_M, PIT_BOX_GUIDE_S, PIT_LIMIT_M_PER_S, STOP_ROLL_CHANCES,
    STOP_ROLL_STATIONARY_S, TYRE_LIFE_NEW_PERCENT,
};
use sim::{
    step, stop_roll, BotLevel, Item, KartInput, KartReadout, RaceEvent, State, Stop, StopTier,
    Track, Tyres,
};

/// Where a kart starts on the lane: on the limiter, short of the first catch.
const START_M: f32 = 40.0;

/// A race of `karts` humans on `sidecar`, past its countdown.
fn race_on(sidecar: &str, karts: usize, tyres: Tyres) -> State {
    let mut state = State::with_tyres(SEED, karts, 0, track(sidecar), BotLevel::Basic, tyres);
    while !matches!(state.phase(), sim::Phase::Racing { .. }) {
        state = step(state, &vec![KartInput::default(); karts], DT);
    }
    state
}

/// One kart started on the lane, driven down it until its stop ends or
/// `ticks` run out: every readout along the way, the first after the start.
fn drive_to_green(sidecar: &str, tyres: Tyres, ticks: usize) -> Vec<KartReadout> {
    let mut state = race_on(sidecar, 1, tyres);
    let mut seen = Vec::new();
    let mut stopped = false;
    for _ in 0..ticks {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        seen.push(kart);
        stopped |= kart.stop.is_some();
        if stopped && kart.stop.is_none() {
            break;
        }
    }
    seen
}

fn stops(seen: &[KartReadout]) -> Vec<Stop> {
    seen.iter().filter_map(|kart| kart.stop).collect()
}

#[test]
fn a_kart_on_tyres_on_is_taken_onto_its_mark_at_the_catch_and_let_go_on_green_with_new_tyres() {
    let sidecar = with_slot_on_lane(&quiet_bell_deer(), 0, START_M);
    let course = track(&sidecar);
    let lane = course.pit_lane().expect("a lane");
    let mark = lane.pit_boxes()[0];
    let seen = drive_to_green(&sidecar, Tyres::On, 2400);
    let caught = seen
        .iter()
        .position(|kart| kart.stop.is_some())
        .expect("the kart was taken in");
    let before = &seen[caught - 1];
    assert!(
        before.tyre_life < TYRE_LIFE_NEW_PERCENT,
        "rolling down the lane wore the tyres: {}",
        before.tyre_life
    );
    let catch_m = lane.pit_box_m(0) - PIT_BOX_CATCH_M;
    let at_catch = lane.reading(seen[caught].position).expect("on the lane");
    assert!(
        lane.reading(before.position).expect("on the lane") < catch_m && at_catch >= catch_m,
        "taken in as it crossed the catch, at {at_catch} m"
    );
    assert!(before.in_pit_lane, "on the limiter at the catch");

    let (tier, stationary_s) = stop_roll(SEED, 0, 0);
    let first = seen[caught].stop.expect("stopping");
    assert_eq!(first.tier, tier);
    assert_eq!(first.stationary_s, stationary_s);
    assert!(!first.on_the_mark(), "the guide comes first");
    assert_eq!(first.stationary_so_far_s, 0.0);

    // The guide lands it on the mark, facing down the lane, and the crew's
    // clock starts there.
    let guide_ticks = (PIT_BOX_GUIDE_S / DT).round() as usize;
    let on_the_mark = seen[caught + guide_ticks];
    let stop = on_the_mark.stop.expect("still stopping");
    assert!(stop.on_the_mark(), "{stop:?}");
    for axis in [0, 2] {
        assert!(
            (on_the_mark.position[axis] - mark.position[axis]).abs() < 0.01,
            "on the mark: {:?} against {:?}",
            on_the_mark.position,
            mark.position
        );
    }
    let yaw = common::yaw(on_the_mark.rotation);
    assert!(
        (yaw - mark.yaw).abs() < 0.01,
        "facing down the lane: {yaw} against {}",
        mark.yaw
    );
    assert_eq!(on_the_mark.speed, 0.0);
    assert!(
        on_the_mark.in_pit_lane,
        "the mark is on the limiter's ground"
    );

    // Held there, dead to the throttle and the steering, until green.
    let last = seen.last().expect("some ticks");
    assert_eq!(last.stop, None, "let go");
    let held = &seen[caught + guide_ticks..seen.len() - 1];
    let expected_ticks = ((PIT_BOX_GUIDE_S + stationary_s) / DT).ceil() as usize;
    assert!(
        (expected_ticks - 1..=expected_ticks + 1).contains(&stops(&seen).len()),
        "stopping for {} ticks, the roll's {expected_ticks}",
        stops(&seen).len()
    );
    for kart in held {
        assert_eq!(kart.position, on_the_mark.position, "held on the mark");
        assert_eq!(kart.speed, 0.0);
        assert!(kart.stop.expect("stopping").on_the_mark());
    }
    let timers: Vec<f32> = held
        .iter()
        .map(|kart| kart.stop.expect("stopping").stationary_so_far_s)
        .collect();
    assert!(
        timers.windows(2).all(|pair| pair[1] > pair[0]),
        "the timer counts up on the mark: {timers:?}"
    );
    assert_eq!(last.tyre_life, TYRE_LIFE_NEW_PERCENT, "new tyres on green");
    assert_eq!(last.position, on_the_mark.position, "let go where it stood");
    assert_eq!(last.speed, 0.0, "at rest on green");
    assert_eq!(last.boost_s(), None);
}

#[test]
fn green_hands_the_controls_back_and_the_stop_is_counted_in_the_standings() {
    let sidecar = with_slot_on_lane(&quiet_bell_deer(), 0, START_M);
    let mut state = race_on(&sidecar, 1, Tyres::On);
    let mut stopped = false;
    for _ in 0..2400 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        stopped |= kart.stop.is_some();
        if stopped && kart.stop.is_none() {
            break;
        }
    }
    assert!(stopped, "the kart stopped");
    let (_, stationary_s) = stop_roll(SEED, 0, 0);
    let standing = &state.standings()[0];
    assert_eq!(standing.stops, 1);
    assert_eq!(standing.best_stop_s, Some(stationary_s));

    let mark = state.kart_readouts()[0].position;
    for _ in 0..120 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        assert_eq!(kart.stop, None, "not taken in again on the way out");
        assert!(
            kart.speed <= PIT_LIMIT_M_PER_S + 0.001,
            "still on the limiter"
        );
    }
    let kart = state.kart_readouts()[0];
    assert!(kart.speed > 5.0, "driving on at {} m/s", kart.speed);
    assert!(
        (kart.position[0] - mark[0]).powi(2) + (kart.position[2] - mark[2]).powi(2) > 25.0,
        "well clear of the mark"
    );
    assert_eq!(state.standings()[0].stops, 1, "counted once");
}

#[test]
fn on_tyres_off_the_pit_boxes_do_nothing() {
    let sidecar = with_slot_on_lane(&quiet_bell_deer(), 0, START_M);
    let course = track(&sidecar);
    let lane = course.pit_lane().expect("a lane");
    let seen = drive_to_green(&sidecar, Tyres::Off, 900);
    assert!(stops(&seen).is_empty(), "no stop on Tyres Off");
    let furthest = seen
        .iter()
        .filter_map(|kart| lane.reading(kart.position))
        .fold(0.0, f32::max);
    assert!(
        furthest > lane.pit_box_m(0) + 10.0,
        "drove past the pit box, to {furthest} m"
    );
}

#[test]
fn the_roll_repeats_for_the_seed_the_slot_and_the_stop_count() {
    assert_eq!(stop_roll(SEED, 0, 0), stop_roll(SEED, 0, 0));
    let first = drive_to_green(
        &with_slot_on_lane(&quiet_bell_deer(), 0, START_M),
        Tyres::On,
        2400,
    );
    let again = drive_to_green(
        &with_slot_on_lane(&quiet_bell_deer(), 0, START_M),
        Tyres::On,
        2400,
    );
    assert_eq!(stops(&first), stops(&again), "the same race stops the same");
    // Another seed, slot or stop count rolls another stop, as a rule: of a
    // handful, at least one differs in each.
    assert!((1..8).any(|seed| stop_roll(seed, 0, 0) != stop_roll(SEED, 0, 0)));
    assert!((1..8).any(|slot| stop_roll(SEED, slot, 0) != stop_roll(SEED, 0, 0)));
    assert!((1..8).any(|count| stop_roll(SEED, 0, count) != stop_roll(SEED, 0, 0)));
}

#[test]
fn ten_thousand_seeds_roll_the_chance_table_within_its_tiers_times() {
    const SEEDS: u64 = 10_000;
    /// Percentage points a tier's share may sit from its chance.
    const TOLERANCE: f32 = 1.5;
    let mut counts: BTreeMap<u8, u32> = BTreeMap::new();
    for seed in 0..SEEDS {
        let (tier, stationary_s) = stop_roll(seed, (seed % 8) as usize, (seed % 3) as u32);
        let [from, to] = STOP_ROLL_STATIONARY_S[usize::from(tier.code())];
        assert!(
            (from..to).contains(&stationary_s),
            "{tier:?} for {stationary_s} s"
        );
        *counts.entry(tier.code()).or_default() += 1;
    }
    for (tier, chance) in StopTier::ALL.into_iter().zip(STOP_ROLL_CHANCES) {
        let share = counts.get(&tier.code()).copied().unwrap_or(0) as f32 / SEEDS as f32;
        assert!(
            (share * 100.0 - chance * 100.0).abs() <= TOLERANCE,
            "{tier:?} rolled {:.1}% against {:.0}%",
            share * 100.0,
            chance * 100.0
        );
    }
}

#[test]
fn the_item_rolls_are_unchanged_by_a_stop() {
    // An item box on the lane past the pit box: the kart that stops reaches it
    // seconds after the kart that drives through, on the same seed.
    let boxed = |tyres: Tyres| {
        let sidecar = with_slot_on_lane(&quiet_bell_deer(), 0, START_M);
        let course = track(&sidecar);
        let lane = course.pit_lane().expect("a lane");
        let (item_box, _) = along_lane(&course, lane.pit_box_m(0) + 20.0);
        let sidecar = with_boxes_at(&sidecar, [item_box]);
        let mut state = race_on(&sidecar, 1, tyres);
        let mut stopped = false;
        for _ in 0..2400 {
            let input = drive_lane(&state, 0);
            state = step(state, &[input], DT);
            stopped |= state.kart_readouts()[0].stop.is_some();
            let rolled = state.events().iter().find_map(|event| match event {
                RaceEvent::ItemPicked {
                    item: Some(item), ..
                } => Some(*item),
                _ => None,
            });
            if let Some(item) = rolled {
                return (stopped, item);
            }
        }
        panic!("the kart never reached the item box");
    };
    let (stopped, after_a_stop): (bool, Item) = boxed(Tyres::On);
    let (drove_through, without_one): (bool, Item) = boxed(Tyres::Off);
    assert!(stopped && !drove_through);
    assert_eq!(
        after_a_stop, without_one,
        "the roll drew nothing from the item generator"
    );
}

#[test]
fn a_kart_on_its_mark_cannot_be_shunted_and_the_other_kart_passes_through_it() {
    // Slot 0 starts just short of its catch; slot 1 starts behind it with
    // its own pit box moved to the end of the lane, so it drives at the
    // limit straight through slot 0's mark.
    let sidecar = {
        let base = quiet_bell_deer();
        let lane = track(&base).pit_lane().expect("a lane").clone();
        let sidecar = with_pit_box_at(&base, 1, lane.exit_m() - 3.0);
        let sidecar = with_slot_on_lane(&sidecar, 0, lane.pit_box_m(0) - PIT_BOX_CATCH_M - 2.0);
        with_slot_on_lane(&sidecar, 1, lane.pit_box_m(0) - 30.0)
    };
    let mut state = race_on(&sidecar, 2, Tyres::On);
    let lane = state.track().pit_lane().expect("a lane").clone();
    let mark_m = lane.pit_box_m(0);
    let mark_at = lane.pit_boxes()[0].position;
    let mut on_the_mark: Option<[f32; 3]> = None;
    let mut passed_through = false;
    let mut closest = f32::MAX;
    for _ in 0..600 {
        // The other kart aims for the mark itself until it is past it.
        let other_m = lane
            .reading(state.kart_readouts()[1].position)
            .unwrap_or(0.0);
        let other = if other_m < mark_m {
            steer_toward(&state, 1, [mark_at[0], mark_at[2]])
        } else {
            drive_lane(&state, 1)
        };
        let inputs = [drive_lane(&state, 0), other];
        state = step(state, &inputs, DT);
        let [held, other] = [state.kart_readouts()[0], state.kart_readouts()[1]];
        let Some(stop) = held.stop else {
            if on_the_mark.is_some() {
                break;
            }
            continue;
        };
        if !stop.on_the_mark() {
            continue;
        }
        let mark = *on_the_mark.get_or_insert(held.position);
        assert_eq!(
            held.position, mark,
            "held on the mark whatever runs into it"
        );
        assert_eq!(held.bump, None, "nothing bumps it");
        let gap =
            ((other.position[0] - mark[0]).powi(2) + (other.position[2] - mark[2]).powi(2)).sqrt();
        closest = closest.min(gap);
        assert_eq!(other.bump, None, "and it bumps nobody");
        passed_through |= lane
            .reading(other.position)
            .is_some_and(|m| m > mark_m + 5.0);
    }
    assert!(on_the_mark.is_some(), "slot 0 stood on its mark");
    assert!(
        closest < KART_HALF_LENGTH_M / 2.0,
        "the other kart drove through the mark, closest {closest:.2} m"
    );
    assert!(passed_through, "and carried on down the lane");
}

#[test]
fn a_stop_is_part_of_the_hash() {
    let sidecar = with_slot_on_lane(&quiet_bell_deer(), 0, START_M);
    let mut state = race_on(&sidecar, 1, Tyres::On);
    for _ in 0..2400 {
        let input = drive_lane(&state, 0);
        state = step(state, &[input], DT);
        if state.kart_readouts()[0]
            .stop
            .is_some_and(|stop| stop.on_the_mark())
        {
            break;
        }
    }
    assert!(state.kart_readouts()[0].stop.is_some(), "on the mark");
    let before = state.hash();
    // Nothing moves on the mark: only the stop's clock tells the ticks apart.
    let state = step(state, &[KartInput::default()], DT);
    assert_eq!(state.kart_readouts()[0].speed, 0.0);
    assert_ne!(before, state.hash());
}

#[test]
fn a_pit_box_off_the_lanes_road_is_refused_with_the_sidecar() {
    let sidecar = quiet_bell_deer();
    let course = track(&sidecar);
    let lane = course.pit_lane().expect("a lane");
    // Well across from the lane, out on the course.
    let (on_the_path, yaw) = along_lane(&course, lane.pit_box_m(0));
    let off_the_road = [on_the_path[0] + 40.0, on_the_path[1], on_the_path[2] + 40.0];
    let mut edited: serde_json::Value = serde_json::from_str(&sidecar).expect("JSON");
    edited["pit_lane"]["pit_boxes"][3] =
        serde_json::json!({ "position": off_the_road, "yaw": yaw });
    let Err(error) = Track::from_sidecar(&edited.to_string()) else {
        panic!("a pit box off the road was accepted");
    };
    assert!(
        error.to_string().contains("pit box"),
        "names the pit box: {error}"
    );
}
