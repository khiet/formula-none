//! The race's lifecycle at the sim's public seam: the countdown that holds
//! the karts on the grid, the rocket start it decides at Go, the finish, and
//! the grace that ends the race after the first finisher. Tests drive karts
//! on the oval fixture and read `phase` and `standings`.

mod common;

use common::{follow_centreline, oval, straight, throttle, ticks, DT, SEED};

use sim::tuning::{
    COUNTDOWN_S, LAPS_PER_RACE, RACE_END_GRACE_S, START_BOOST_FACTOR, START_BOOST_S,
    START_FULL_BOOST_HOLD_S, START_STALL_HOLD_S, START_STALL_S, START_THROTTLE_HELD,
    START_WEAK_BOOST_FACTOR, START_WEAK_BOOST_HOLD_S, START_WEAK_BOOST_S, TOP_SPEED_M_PER_S,
};
use sim::{step, KartInput, Phase, Start, State};

fn race(kart_count: usize) -> State {
    State::new(SEED, kart_count, oval())
}

fn idle(_: &State, _: usize) -> KartInput {
    KartInput::default()
}

type Driver = fn(&State, usize) -> KartInput;

fn inputs(state: &State, drivers: &[Driver]) -> Vec<KartInput> {
    drivers
        .iter()
        .enumerate()
        .map(|(kart, driver)| driver(state, kart))
        .collect()
}

/// Step every kart by its own driver for `seconds`.
fn drive(mut state: State, drivers: &[Driver], seconds: f32) -> State {
    for _ in 0..ticks(seconds) {
        let inputs = inputs(&state, drivers);
        state = step(state, &inputs, DT);
    }
    state
}

/// Step every kart by its own driver until `done`, or panic after `seconds`.
fn drive_until(
    mut state: State,
    drivers: &[Driver],
    seconds: f32,
    done: impl Fn(&State) -> bool,
) -> State {
    for _ in 0..ticks(seconds) {
        if done(&state) {
            return state;
        }
        let inputs = inputs(&state, drivers);
        state = step(state, &inputs, DT);
    }
    panic!("not done within {seconds} s");
}

fn remaining(state: &State) -> f32 {
    match state.phase() {
        Phase::Countdown { remaining_s } => remaining_s,
        phase => panic!("the countdown is over: {phase:?}"),
    }
}

fn racing(state: &State) -> bool {
    matches!(state.phase(), Phase::Racing { .. })
}

/// The race past its countdown with every kart idle on the grid.
fn started(state: State) -> State {
    let drivers = vec![idle as Driver; state.human_count()];
    drive_until(state, &drivers, COUNTDOWN_S + 1.0, racing)
}

#[test]
fn the_karts_wait_on_the_grid_in_slot_order_through_the_countdown_then_go() {
    // Bots drive themselves, so the countdown must hold them too.
    let mut state = State::with_bots(SEED, 1, 2, oval());
    assert_eq!(
        state.phase(),
        Phase::Countdown {
            remaining_s: COUNTDOWN_S
        }
    );
    let grid = state.kart_readouts();
    let slots = state.track().spawn_grid().to_vec();
    let mut countdown_ticks = 0;
    while !racing(&state) {
        state = step(state, &[throttle()], DT);
        countdown_ticks += 1;
        assert!(countdown_ticks <= ticks(COUNTDOWN_S) + 1, "no Go");
    }
    assert!(
        countdown_ticks >= ticks(COUNTDOWN_S) - 1,
        "Go came after {countdown_ticks} ticks"
    );
    for ((kart, slot), before) in state.kart_readouts().iter().zip(&slots).zip(&grid) {
        assert_eq!(kart.position[0], slot.position[0]);
        assert_eq!(kart.position[2], slot.position[2]);
        assert_eq!(kart.position, before.position, "held on the grid");
        assert_eq!(kart.speed, 0.0);
    }
    for standing in state.standings() {
        assert_eq!(standing.lap_time_s, 0.0, "the lap clock starts at Go");
        assert_eq!(standing.laps_completed, 0);
    }
    assert_eq!(
        state.phase(),
        Phase::Racing {
            elapsed_s: 0.0,
            grace_left_s: None
        }
    );
}

#[test]
fn the_countdown_counts_down_by_the_tick() {
    let state = step(race(1), &[KartInput::default()], DT);
    assert!((remaining(&state) - (COUNTDOWN_S - DT)).abs() < 1e-5);
}

/// The race at Go after a throttle hold that began at the tick whose count
/// stood at `hold_s` and was held through Go: a hold of `hold_s`.
fn held_through_go(hold_s: f32, held: KartInput) -> State {
    let state = drive_until(race(1), &[idle], COUNTDOWN_S, |state| {
        remaining(state) <= hold_s + DT / 2.0
    });
    let mut state = state;
    while !racing(&state) {
        state = step(state, &[held], DT);
    }
    state
}

/// `held_through_go` on the full throttle.
fn throttle_through_go(hold_s: f32) -> State {
    held_through_go(hold_s, throttle())
}

/// The kart's speed the tick after Go under full throttle: the boosted top
/// speed off the line for a boost, nothing for a stall, a crawl for a clean
/// start.
fn off_the_line(state: State) -> f32 {
    step(state, &[throttle()], DT).kart_readouts()[0].speed
}

/// The speed after `seconds` of full throttle from `state`.
fn speed_after(state: State, seconds: f32) -> f32 {
    drive(state, &[straight], seconds).kart_readouts()[0].speed
}

#[test]
fn a_hold_from_the_first_half_of_the_two_through_go_gives_the_full_boost() {
    // Pressed as the 2 appeared, or as late as the full edge.
    for hold_s in [START_STALL_HOLD_S, START_FULL_BOOST_HOLD_S] {
        let state = throttle_through_go(hold_s);
        assert_eq!(state.kart_readouts()[0].start, Start::FullBoost);
        let state = step(state, &[throttle()], DT);
        assert_eq!(
            state.kart_readouts()[0].speed,
            TOP_SPEED_M_PER_S * START_BOOST_FACTOR,
            "a hold of {hold_s} s is off the line at the boosted top speed"
        );
        let state = drive(state, &[straight], START_BOOST_S - 2.0 * DT);
        assert_eq!(
            state.kart_readouts()[0].speed,
            TOP_SPEED_M_PER_S * START_BOOST_FACTOR,
            "still boosting just before the tuned time"
        );
        assert!(
            speed_after(state, 3.0 * DT) <= TOP_SPEED_M_PER_S,
            "back under top speed once the boost is spent"
        );
    }
}

#[test]
fn a_hold_from_the_second_half_of_the_two_through_go_gives_the_weak_boost() {
    // Pressed just after the full edge, or as late as the weak edge.
    for hold_s in [START_FULL_BOOST_HOLD_S - DT, START_WEAK_BOOST_HOLD_S] {
        let state = throttle_through_go(hold_s);
        assert_eq!(state.kart_readouts()[0].start, Start::WeakBoost);
        let state = step(state, &[throttle()], DT);
        assert_eq!(
            state.kart_readouts()[0].speed,
            TOP_SPEED_M_PER_S * START_WEAK_BOOST_FACTOR,
            "a hold of {hold_s} s is off the line at the weak boost's top speed"
        );
        let state = drive(state, &[straight], START_WEAK_BOOST_S - 2.0 * DT);
        assert_eq!(
            state.kart_readouts()[0].speed,
            TOP_SPEED_M_PER_S * START_WEAK_BOOST_FACTOR,
            "still boosting just before the tuned time"
        );
        assert!(
            speed_after(state, 3.0 * DT) <= TOP_SPEED_M_PER_S,
            "back under top speed once the boost is spent"
        );
    }
}

#[test]
fn a_hold_from_the_one_or_later_starts_clean() {
    let state = throttle_through_go(START_WEAK_BOOST_HOLD_S - DT);
    assert_eq!(state.kart_readouts()[0].start, Start::Clean);
    let speed = off_the_line(state);
    assert!(
        0.0 < speed && speed < 1.0,
        "a crawl off the line: {speed} m/s"
    );
    // The same crawl as a start with no hold.
    let clean = started(race(1));
    assert_eq!(off_the_line(clean), speed);
}

#[test]
fn a_hold_released_before_go_starts_clean_whatever_its_length() {
    // Held from the 2 appearing and let go on the countdown's last tick.
    let let_go_at_go: Driver = |state, _| match state.phase() {
        Phase::Countdown { remaining_s } if remaining_s > START_STALL_HOLD_S => idle(state, 0),
        Phase::Countdown { remaining_s } if remaining_s < 1.5 * DT => idle(state, 0),
        _ => throttle(),
    };
    let state = drive_until(race(1), &[let_go_at_go], COUNTDOWN_S + 1.0, racing);
    assert_eq!(state.kart_readouts()[0].start, Start::Clean);
    let speed = off_the_line(state);
    assert!(
        0.0 < speed && speed < 1.0,
        "a crawl off the line: {speed} m/s"
    );
}

#[test]
fn a_hold_from_before_the_two_stalls_the_kart_for_the_tuned_time_dead_to_every_control() {
    // Pressed one tick before the 2 appeared.
    let state = throttle_through_go(START_STALL_HOLD_S + DT);
    assert_eq!(state.kart_readouts()[0].start, Start::Stall);
    let grid = state.kart_readouts()[0];
    // Nor does the brake back a stalled kart off the grid.
    let brake: Driver = |_, _| KartInput {
        brake: 1.0,
        ..KartInput::default()
    };
    let state = drive(state, &[brake], START_STALL_S / 2.0);
    let state = drive(state, &[straight], START_STALL_S / 2.0 - 2.0 * DT);
    let stalled = state.kart_readouts()[0];
    assert_eq!(stalled.speed, 0.0, "stalled just before the tuned time");
    assert_eq!(stalled.position[0], grid.position[0]);
    assert_eq!(stalled.position[2], grid.position[2]);
    let left = stalled.stall_s.expect("the stall's time left is reported");
    assert!(0.0 < left && left < 3.0 * DT, "{left} s of stall left");

    let state = drive(state, &[straight], 0.5);
    let going = state.kart_readouts()[0];
    assert_eq!(going.stall_s, None);
    assert!(going.speed > 1.0, "driving again: {} m/s", going.speed);
    assert!(
        going.position[2] < grid.position[2] - 0.5,
        "moved off the grid"
    );

    // A clean start is well ahead by the time the stall is over.
    let clean = drive(started(race(1)), &[straight], START_STALL_S + 0.5);
    assert!(
        clean.kart_readouts()[0].speed > going.speed + 5.0,
        "a clean start is well ahead: {} m/s against {} m/s",
        clean.kart_readouts()[0].speed,
        going.speed
    );
}

#[test]
fn a_release_before_go_resets_the_hold() {
    // Held from before the 2, let go for one tick in the second half of
    // the 2, then held through Go: only the second hold counts.
    const LET_GO_AT_S: f32 = 1.3;
    let let_go_once: Driver = |state, _| match state.phase() {
        Phase::Countdown { remaining_s } if remaining_s > START_STALL_HOLD_S + 0.5 => {
            idle(state, 0)
        }
        Phase::Countdown { remaining_s } if (remaining_s - LET_GO_AT_S).abs() < DT / 2.0 => {
            idle(state, 0)
        }
        _ => throttle(),
    };
    let state = drive_until(race(1), &[let_go_once], COUNTDOWN_S + 1.0, racing);
    assert_eq!(state.kart_readouts()[0].start, Start::WeakBoost);
    assert_eq!(
        off_the_line(state),
        TOP_SPEED_M_PER_S * START_WEAK_BOOST_FACTOR
    );
}

#[test]
fn the_throttle_counts_as_held_only_above_the_threshold() {
    let resting = KartInput {
        throttle: START_THROTTLE_HELD,
        ..KartInput::default()
    };
    let state = held_through_go(START_FULL_BOOST_HOLD_S, resting);
    assert_eq!(state.kart_readouts()[0].start, Start::Clean);

    let pressed = KartInput {
        throttle: START_THROTTLE_HELD + 0.01,
        ..KartInput::default()
    };
    let state = held_through_go(START_FULL_BOOST_HOLD_S, pressed);
    assert_eq!(state.kart_readouts()[0].start, Start::FullBoost);
}

#[test]
fn the_tier_the_hold_so_far_would_earn_is_reported_as_it_charges() {
    // Pressed as the 3 appeared and held: the tiers come as the hold
    // lengthens, and a hold that has tipped into a stall reads as one.
    let mut state = race(1);
    let mut seen = Vec::new();
    while !racing(&state) {
        state = step(state, &[throttle()], DT);
        let start = state.kart_readouts()[0].start;
        if seen.last() != Some(&start) {
            seen.push(start);
        }
    }
    assert_eq!(
        seen,
        [
            Start::Clean,
            Start::WeakBoost,
            Start::FullBoost,
            Start::Stall
        ]
    );
}

/// A lap at top speed takes about 15 s.
const LAP_S: f32 = 20.0;

#[test]
fn a_lone_kart_finishing_its_laps_ends_the_race_at_once() {
    let state = started(race(1));
    let state = drive_until(
        state,
        &[follow_centreline],
        LAPS_PER_RACE as f32 * LAP_S,
        |state| state.standings()[0].laps_completed == LAPS_PER_RACE,
    );
    assert_eq!(state.phase(), Phase::Finished);
    let standing = state.standings()[0];
    let finish = standing.finish_time_s.expect("the kart finished");
    assert!(
        (30.0..LAPS_PER_RACE as f32 * LAP_S).contains(&finish),
        "finish time {finish} s"
    );
    assert_eq!(standing.position, 1);

    // Nothing moves once the race is over.
    let hash = state.hash();
    let transforms = state.kart_readouts();
    let state = drive(state, &[straight], 1.0);
    assert_eq!(state.hash(), hash);
    assert_eq!(state.kart_readouts(), transforms);
    assert_eq!(state.standings()[0].finish_time_s, Some(finish));
}

#[test]
fn the_race_ends_the_tuned_grace_after_the_first_finisher_and_places_the_rest_by_progress() {
    // Pole drives up its column for a second and stops; the kart in the
    // other column laps the oval, passing it clean; the third never leaves
    // the grid.
    let state = started(race(3));
    let state = drive(state, &[straight, follow_centreline, idle], 1.0);
    let state = drive_until(
        state,
        &[idle, follow_centreline, idle],
        LAPS_PER_RACE as f32 * LAP_S,
        |state| state.standings()[1].laps_completed == LAPS_PER_RACE,
    );
    let first_finish = state.standings()[1]
        .finish_time_s
        .expect("the lapping kart finished");
    let Phase::Racing {
        elapsed_s,
        grace_left_s: Some(grace_left_s),
    } = state.phase()
    else {
        panic!(
            "the grace runs from the first finisher: {:?}",
            state.phase()
        );
    };
    assert!((grace_left_s - RACE_END_GRACE_S).abs() < 1e-5);
    assert!(
        (elapsed_s - first_finish).abs() <= DT,
        "finished at {first_finish} s, {elapsed_s} s into the race"
    );

    // Just short of the grace the race is still on; within a tick or two
    // of it, the timer's rounding allowed for, it is over.
    let state = drive(state, &[idle, idle, idle], RACE_END_GRACE_S - 2.0 * DT);
    assert!(racing(&state), "{:?}", state.phase());
    let state = drive(state, &[idle, idle, idle], 3.0 * DT);
    assert_eq!(state.phase(), Phase::Finished);

    let standings = state.standings();
    let positions: Vec<u32> = standings.iter().map(|standing| standing.position).collect();
    assert_eq!(positions, [2, 1, 3]);
    assert_eq!(standings[1].finish_time_s, Some(first_finish));
    assert_eq!(standings[0].finish_time_s, None);
    assert_eq!(standings[2].finish_time_s, None);
    assert_eq!(standings[0].laps_completed, 0);
    let lap = state.track().spline_length();
    assert!(
        standings[0].spline_progress < 60.0,
        "the stopped kart is past the line at {} m",
        standings[0].spline_progress
    );
    assert!(
        standings[2].spline_progress > lap - 30.0,
        "the kart on the grid is behind the line at {} m",
        standings[2].spline_progress
    );
}

#[test]
fn finishing_order_is_final_whatever_a_finisher_does_after_the_line() {
    // Pole finishes and backs over the line again, curving out of the
    // racing line; second, held on the grid for a while, finishes later
    // and is just past the line, ahead of pole by progress alone.
    let state = started(race(2));
    let state = drive(state, &[follow_centreline, idle], 8.0);
    let state = drive_until(
        state,
        &[follow_centreline, follow_centreline],
        LAPS_PER_RACE as f32 * LAP_S,
        |state| state.standings()[0].laps_completed == LAPS_PER_RACE,
    );
    assert_eq!(state.standings()[0].position, 1);
    let best = state.standings()[0].best_lap_s;
    let lap_time = state.standings()[0].lap_time_s;
    // Backing up with a little steer curves the kart out of the racing line.
    let reverse_aside: Driver = |_, _| KartInput {
        brake: 1.0,
        steer: 0.08,
        ..KartInput::default()
    };
    let state = drive(state, &[reverse_aside, follow_centreline], 5.5);
    let state = drive_until(state, &[idle, follow_centreline], LAP_S, |state| {
        state.standings()[1].laps_completed == LAPS_PER_RACE
    });
    assert_eq!(state.phase(), Phase::Finished, "everyone finished");
    let standings = state.standings();
    assert!(
        state
            .track()
            .lap_relative_progress(0, standings[0].spline_progress)
            < state
                .track()
                .lap_relative_progress(0, standings[1].spline_progress),
        "pole backed up behind the line: {} m against {} m",
        standings[0].spline_progress,
        standings[1].spline_progress
    );
    assert_eq!(standings[0].position, 1);
    assert_eq!(standings[1].position, 2);
    assert!(standings[0].finish_time_s < standings[1].finish_time_s);
    assert_eq!(standings[0].laps_completed, LAPS_PER_RACE, "no fourth lap");
    assert_eq!(standings[0].best_lap_s, best, "the best lap is a race lap");
    assert_eq!(
        standings[0].lap_time_s, lap_time,
        "the lap clock stops at the finish"
    );
}
