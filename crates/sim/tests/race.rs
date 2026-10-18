//! The race's lifecycle at the sim's public seam: the countdown that holds
//! the karts on the grid, the start boost and the stall, the finish, and
//! the grace that ends the race after the first finisher. Tests drive karts
//! on the oval fixture and read `phase` and `standings`.

use sim::tuning::{
    BOT_LOOKAHEAD_M, COUNTDOWN_S, LAPS_PER_RACE, RACE_END_GRACE_S, START_BOOST_FACTOR,
    START_BOOST_S, START_BOOST_WINDOW_S, START_STALL_S, TICKS_PER_SECOND, TOP_SPEED_M_PER_S,
};
use sim::{step, KartInput, Phase, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;

fn oval() -> Track {
    Track::from_sidecar(OVAL).expect("the oval fixture loads")
}

fn race(kart_count: usize) -> State {
    State::new(SEED, kart_count, oval())
}

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

fn throttle() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

fn idle(_: &State, _: usize) -> KartInput {
    KartInput::default()
}

fn straight(_: &State, _: usize) -> KartInput {
    throttle()
}

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`.
fn yaw(rotation: [f32; 4]) -> f32 {
    let [x, y, z, w] = rotation;
    (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z))
}

/// Full throttle, steering toward a line a little right of the centreline
/// some way ahead of the kart: enough to lap the oval on the asphalt and
/// clear of the pipe that stands just left of the centreline in each turn.
fn follow_centreline(state: &State, kart: usize) -> KartInput {
    const RIGHT_OF_CENTRE_M: f32 = 2.5;
    const STEER_GAIN: f32 = 4.0;
    let transform = state.kart_transforms()[kart];
    let progress = state.standings()[kart].spline_progress;
    let track = state.track();
    let ahead = track.point_at(progress + BOT_LOOKAHEAD_M);
    let further = track.point_at(progress + BOT_LOOKAHEAD_M + 1.0);
    let (tx, tz) = (further[0] - ahead[0], further[2] - ahead[2]);
    let travel = (tx * tx + tz * tz).sqrt().max(f32::EPSILON);
    let target = [
        ahead[0] - tz / travel * RIGHT_OF_CENTRE_M,
        ahead[2] + tx / travel * RIGHT_OF_CENTRE_M,
    ];
    let (dx, dz) = (
        target[0] - transform.position[0],
        target[1] - transform.position[2],
    );
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    let (sin, cos) = yaw(transform.rotation).sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    KartInput {
        steer: (rightward * STEER_GAIN).clamp(-1.0, 1.0),
        throttle: 1.0,
        ..KartInput::default()
    }
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
    let grid = state.kart_transforms();
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
    for ((kart, slot), before) in state.kart_transforms().iter().zip(&slots).zip(&grid) {
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

#[test]
fn pressing_accelerate_in_the_last_window_of_the_countdown_gives_the_start_boost() {
    let state = drive_until(race(1), &[idle], COUNTDOWN_S, |state| {
        remaining(state) <= START_BOOST_WINDOW_S
    });
    let state = drive_until(state, &[straight], 1.0, racing);
    let state = step(state, &[throttle()], DT);
    assert_eq!(
        state.kart_transforms()[0].speed,
        TOP_SPEED_M_PER_S * START_BOOST_FACTOR,
        "off the line at the boosted top speed"
    );

    let state = drive(state, &[straight], START_BOOST_S - 2.0 * DT);
    assert_eq!(
        state.kart_transforms()[0].speed,
        TOP_SPEED_M_PER_S * START_BOOST_FACTOR,
        "still boosting just before the tuned time"
    );
    let state = drive(state, &[straight], 3.0 * DT);
    assert!(
        state.kart_transforms()[0].speed <= TOP_SPEED_M_PER_S,
        "back under top speed once the boost is spent: {} m/s",
        state.kart_transforms()[0].speed
    );
}

#[test]
fn pressing_accelerate_before_the_window_stalls_the_kart_for_the_tuned_time() {
    // The press comes at the first tick and is held through Go.
    let state = drive_until(race(1), &[straight], COUNTDOWN_S + 1.0, racing);
    let grid = state.kart_transforms()[0];
    // Nor does the brake back a stalled kart off the grid.
    let brake: Driver = |_, _| KartInput {
        brake: 1.0,
        ..KartInput::default()
    };
    let state = drive(state, &[brake], START_STALL_S / 2.0);
    let state = drive(state, &[straight], START_STALL_S / 2.0 - 2.0 * DT);
    let stalled = state.kart_transforms()[0];
    assert_eq!(stalled.speed, 0.0, "stalled just before the tuned time");
    assert_eq!(stalled.position[0], grid.position[0]);
    assert_eq!(stalled.position[2], grid.position[2]);

    let state = drive(state, &[straight], 0.5);
    let going = state.kart_transforms()[0];
    assert!(going.speed > 1.0, "driving again: {} m/s", going.speed);
    assert!(
        going.position[2] < grid.position[2] - 0.5,
        "moved off the grid"
    );

    // A clean start is well ahead by the time the stall is over.
    let clean = drive(started(race(1)), &[straight], START_STALL_S + 0.5);
    assert!(
        clean.kart_transforms()[0].speed > going.speed + 5.0,
        "a clean start is well ahead: {} m/s against {} m/s",
        clean.kart_transforms()[0].speed,
        going.speed
    );
}

#[test]
fn a_press_too_early_is_not_undone_by_another_in_the_window() {
    let early_then_window: Driver = |state, _| {
        if matches!(state.phase(), Phase::Countdown { remaining_s } if remaining_s > START_BOOST_WINDOW_S + 0.5)
        {
            throttle()
        } else if matches!(state.phase(), Phase::Countdown { .. }) {
            KartInput::default()
        } else {
            throttle()
        }
    };
    let state = drive_until(
        race(1),
        &[early_then_window],
        COUNTDOWN_S + 1.0,
        |state| matches!(state.phase(), Phase::Countdown { remaining_s } if remaining_s <= START_BOOST_WINDOW_S),
    );
    let state = drive_until(state, &[straight], 1.0, racing);
    let state = drive(state, &[straight], START_STALL_S / 2.0);
    assert_eq!(state.kart_transforms()[0].speed, 0.0, "still stalled");
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
    let transforms = state.kart_transforms();
    let state = drive(state, &[straight], 1.0);
    assert_eq!(state.hash(), hash);
    assert_eq!(state.kart_transforms(), transforms);
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
