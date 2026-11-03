//! The race's lifecycle at the sim's public seam: the countdown that holds
//! the karts on the grid, the rocket start it decides at Go, the finish and
//! the bot that drives a finisher, the end of the race once every human has
//! finished or the cap runs out, and the outro. Tests drive karts
//! on the oval fixture and read `phase` and `standings`.

mod common;

use common::{follow_centreline, oval, straight, throttle, ticks, DT, SEED};

use sim::tuning::{
    COUNTDOWN_S, LAPS_PER_RACE, RACE_END_CAP_S, RACE_OUTRO_S, START_BOOST_FACTOR, START_BOOST_S,
    START_FULL_BOOST_HOLD_S, START_STALL_HOLD_S, START_STALL_S, START_THROTTLE_HELD,
    START_WEAK_BOOST_FACTOR, START_WEAK_BOOST_HOLD_S, START_WEAK_BOOST_S, TOP_SPEED_M_PER_S,
};
use sim::{step, BotLevel, ItemSlot, KartInput, MiniTurbo, Phase, RaceEvent, Start, State};

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

fn in_the_outro(state: &State) -> bool {
    matches!(state.phase(), Phase::Outro { .. })
}

/// The race past its countdown with every kart idle on the grid.
fn started(state: State) -> State {
    let drivers = vec![idle as Driver; state.human_count()];
    drive_until(state, &drivers, COUNTDOWN_S + 1.0, racing)
}

#[test]
fn the_karts_wait_on_the_grid_in_slot_order_through_the_countdown_then_go() {
    // Bots drive themselves, so the countdown must hold them too.
    let mut state = State::with_bots(SEED, 1, 2, oval(), BotLevel::Basic);
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
    // One second per start light, five lights.
    assert!(
        countdown_ticks.abs_diff(ticks(5.0)) <= 1,
        "Go came after {countdown_ticks} ticks, not five seconds"
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
            cap_left_s: None
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
fn a_hold_from_the_first_half_of_the_fourth_light_through_go_gives_the_full_boost() {
    // Pressed as the fourth light came on, or as late as the full edge.
    for hold_s in [START_STALL_HOLD_S, START_FULL_BOOST_HOLD_S] {
        let state = throttle_through_go(hold_s);
        assert_eq!(state.kart_readouts()[0].start, Start::FullBoost);
        let state = step(state, &[throttle()], DT);
        assert_eq!(state.kart_readouts()[0].boost_tier, MiniTurbo::Red);
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
fn a_hold_from_the_second_half_of_the_fourth_light_through_go_gives_the_weak_boost() {
    // Pressed just after the full edge, or as late as the weak edge.
    for hold_s in [START_FULL_BOOST_HOLD_S - DT, START_WEAK_BOOST_HOLD_S] {
        let state = throttle_through_go(hold_s);
        assert_eq!(state.kart_readouts()[0].start, Start::WeakBoost);
        let state = step(state, &[throttle()], DT);
        assert_eq!(state.kart_readouts()[0].boost_tier, MiniTurbo::Orange);
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
fn a_hold_from_the_fifth_light_or_later_starts_clean() {
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
    // Held from the fourth light coming on and let go on the countdown's last tick.
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
fn a_hold_from_before_the_fourth_light_stalls_the_kart_for_the_tuned_time_dead_to_every_control() {
    // Pressed one tick before the fourth light came on.
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
    // Held from before the fourth light, let go for one tick in its second
    // half, then held through Go: only the second hold counts.
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
    // Pressed as the first light came on and held: the tiers come as the hold
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

/// Far enough short of the line that neither bot finishes before both
/// humans do, from this seed.
const BOTS_BEHIND_M: f32 = 350.0;

fn finished(state: &State, kart: usize) -> bool {
    state.standings()[kart].finish_time_s.is_some()
}

/// Drive until `kart` has finished its laps, with time for traffic.
fn drive_until_finished(state: State, drivers: &[Driver], kart: usize) -> State {
    drive_until(
        state,
        drivers,
        LAPS_PER_RACE as f32 * 1.5 * LAP_S,
        |state| finished(state, kart),
    )
}

/// A race of two humans on the oval's grid and two bots `behind_m` short
/// of the start line, either side of the centreline, so the humans finish
/// while the bots are still racing.
fn humans_ahead_of_two_bots(behind_m: f32) -> State {
    let oval = oval();
    let at = oval.spline_length() - behind_m;
    let point = oval.point_at(at);
    let ahead = oval.point_at(at + 1.0);
    let (tx, tz) = (ahead[0] - point[0], ahead[2] - point[2]);
    let travel = (tx * tx + tz * tz).sqrt();
    // The kart faces -Z at yaw 0.
    let yaw = (-tx).atan2(-tz);
    let mut sidecar = common::OVAL.to_string();
    for (slot, right_m) in [(2, -2.0), (3, 2.0)] {
        let position = [
            point[0] - tz / travel * right_m,
            point[1],
            point[2] + tx / travel * right_m,
        ];
        sidecar = common::with_slot_at(&sidecar, slot, position, yaw);
    }
    started(State::with_bots(
        SEED,
        2,
        2,
        common::track(&sidecar),
        BotLevel::Basic,
    ))
}

fn positions(state: &State) -> Vec<u32> {
    state
        .standings()
        .iter()
        .map(|standing| standing.position)
        .collect()
}

#[test]
fn a_lone_kart_finishing_its_laps_ends_the_race_at_once_and_the_outro_runs_down() {
    let state = started(race(1));
    let state = drive_until_finished(state, &[follow_centreline], 0);
    assert_eq!(
        state.phase(),
        Phase::Outro {
            remaining_s: RACE_OUTRO_S
        }
    );
    assert_eq!(
        state.events().last(),
        Some(&RaceEvent::RaceEnded),
        "the end is reported after the finish"
    );
    let standing = state.standings()[0];
    let finish = standing.finish_time_s.expect("the kart finished");
    assert!(
        (30.0..LAPS_PER_RACE as f32 * LAP_S).contains(&finish),
        "finish time {finish} s"
    );
    assert_eq!(standing.position, 1);

    // Through the outro a bot drives the kart on.
    let at_the_line = state.kart_readouts()[0].position;
    let state = drive(state, &[idle], RACE_OUTRO_S - 2.0 * DT);
    assert!(in_the_outro(&state), "{:?}", state.phase());
    assert!(state.is_bot(0));
    assert!(
        state.kart_readouts()[0].position != at_the_line,
        "the bot drives on"
    );
    let state = drive(state, &[idle], 3.0 * DT);
    assert_eq!(state.phase(), Phase::Finished);

    // Nothing moves once the race is over.
    let hash = state.hash();
    let transforms = state.kart_readouts();
    let state = drive(state, &[straight], 1.0);
    assert_eq!(state.hash(), hash);
    assert_eq!(state.kart_readouts(), transforms);
    assert_eq!(state.standings()[0].finish_time_s, Some(finish));
}

#[test]
fn the_race_stays_on_while_a_human_races_whatever_the_bots_have_done() {
    // Pole and both bots lap; the other human never leaves the grid.
    let state = started(State::with_bots(SEED, 2, 2, oval(), BotLevel::Basic));
    let state = drive_until(state, &[follow_centreline, idle], 4.0 * LAP_S, |state| {
        [0, 2, 3].iter().all(|&kart| finished(state, kart))
    });
    assert!(racing(&state), "{:?}", state.phase());
    let state = drive(state, &[idle, idle], 10.0);
    assert!(racing(&state), "{:?}", state.phase());
}

#[test]
fn the_race_ends_as_the_last_human_finishes_with_the_bots_still_racing_placed_by_progress() {
    // The second human sets off four seconds after the first.
    let state = humans_ahead_of_two_bots(BOTS_BEHIND_M);
    let state = drive(state, &[follow_centreline, idle], 4.0);
    let drivers = [follow_centreline as Driver, follow_centreline];
    let state = drive_until_finished(state, &drivers, 0);
    assert!(racing(&state), "{:?}", state.phase());
    let state = drive_until_finished(state, &drivers, 1);
    assert_eq!(
        state.phase(),
        Phase::Outro {
            remaining_s: RACE_OUTRO_S
        }
    );

    let standings = state.standings();
    assert_eq!(&positions(&state)[..2], [1, 2]);
    let bots = &standings[2..];
    assert!(
        bots.iter().all(|bot| bot.finish_time_s.is_none()),
        "the bots are still racing: {bots:?}"
    );
    let distance = |standing: &sim::Standing| {
        standing.laps_completed as f32 * state.track().spline_length()
            + state
                .track()
                .lap_relative_progress(standing.checkpoint, standing.spline_progress)
    };
    let (ahead, behind) = if bots[0].position < bots[1].position {
        (&bots[0], &bots[1])
    } else {
        (&bots[1], &bots[0])
    };
    assert_eq!((ahead.position, behind.position), (3, 4));
    assert!(distance(ahead) > distance(behind));

    // The outro moves the karts, not the results.
    let results = positions(&state);
    let state = drive(state, &drivers, RACE_OUTRO_S + DT);
    assert_eq!(state.phase(), Phase::Finished);
    assert_eq!(positions(&state), results);
    assert!(state.standings()[2..]
        .iter()
        .all(|bot| bot.finish_time_s.is_none()));
}

#[test]
fn the_cap_ends_the_race_the_tuned_time_after_the_first_human_finishes() {
    // Pole laps; the other human never leaves the grid.
    let state = started(race(2));
    let state = drive_until_finished(state, &[follow_centreline, idle], 0);
    let first_finish = state.standings()[0].finish_time_s.expect("pole finished");
    let Phase::Racing {
        elapsed_s,
        cap_left_s: Some(cap_left_s),
    } = state.phase()
    else {
        panic!("the cap runs from the first finish: {:?}", state.phase());
    };
    assert!((cap_left_s - RACE_END_CAP_S).abs() < 1e-5);
    assert!(
        (elapsed_s - first_finish).abs() <= DT,
        "finished at {first_finish} s, {elapsed_s} s into the race"
    );

    // Just short of the cap the race is still on; within a tick or two of
    // it, the timer's rounding allowed for, it is over.
    let state = drive(state, &[idle, idle], RACE_END_CAP_S - 2.0 * DT);
    assert!(racing(&state), "{:?}", state.phase());
    let state = drive(state, &[idle, idle], 3.0 * DT);
    assert!(in_the_outro(&state), "{:?}", state.phase());
    assert_eq!(positions(&state), [1, 2]);
    assert_eq!(state.standings()[1].finish_time_s, None);
    assert!(state.is_bot(1), "a bot drives the outro");
    let mut state = state;
    state.hand_back(1);
    assert!(state.is_bot(1), "the race is over for the human too");
}

#[test]
fn a_kart_handed_to_a_bot_does_not_hold_the_race_open() {
    // A bot laps faster than `follow_centreline`, so it takes over a lap
    // down, with too far to go to finish first.
    let mut state = drive(started(race(2)), &[follow_centreline, idle], 20.0);
    state.hand_to_bot(1);
    let state = drive_until_finished(state, &[follow_centreline, idle], 0);
    assert!(!finished(&state, 1));
    assert!(in_the_outro(&state), "{:?}", state.phase());
}

#[test]
fn a_finish_by_a_kart_handed_to_a_bot_does_not_start_the_cap() {
    // Pole idles on the grid; the other human is gone and their bot laps.
    let mut state = started(race(2));
    state.hand_to_bot(1);
    let state = drive_until_finished(state, &[idle, idle], 1);
    assert!(
        matches!(
            state.phase(),
            Phase::Racing {
                cap_left_s: None,
                ..
            }
        ),
        "the cap waits for a human's own finish: {:?}",
        state.phase()
    );
}

#[test]
fn the_race_ends_at_once_when_every_human_has_gone() {
    let state = started(race(2));
    let mut state = drive(state, &[follow_centreline, follow_centreline], 2.0);
    state.hand_to_bot(0);
    state.hand_to_bot(1);
    let state = drive(state, &[idle, idle], DT);
    assert!(in_the_outro(&state), "{:?}", state.phase());
}

/// Pole's race of two from the top of the countdown, lapping to its finish
/// while the other human waits on the grid, then `after` from both humans
/// for five seconds.
fn finish_then(after: KartInput) -> State {
    let state = started(race(2));
    let mut state = drive_until_finished(state, &[follow_centreline, idle], 0);
    for _ in 0..ticks(5.0) {
        state = step(state, &[after, KartInput::default()], DT);
    }
    state
}

#[test]
fn a_bot_drives_a_finished_kart_and_its_human_inputs_change_nothing() {
    let full_left = KartInput {
        steer: -1.0,
        throttle: 1.0,
        hop: true,
        ..KartInput::default()
    };
    let reversing = KartInput {
        brake: 1.0,
        steer: 1.0,
        ..KartInput::default()
    };
    let one = finish_then(full_left);
    let other = finish_then(reversing);
    assert!(one.is_bot(0));
    assert!(racing(&one), "the other human holds the race open");
    assert_eq!(one.hash(), other.hash());
    assert!(
        one.kart_readouts()[0].speed > 0.5 * TOP_SPEED_M_PER_S,
        "the bot drives on"
    );

    // The human coming back takes nothing back from the bot.
    let mut state = one;
    state.hand_back(0);
    assert!(state.is_bot(0));
}

#[test]
fn a_finished_kart_counts_no_more_laps_and_keeps_its_clocks() {
    let state = started(race(2));
    let state = drive_until_finished(state, &[follow_centreline, idle], 0);
    let finish = state.standings()[0];
    // The bot laps on, over the line again.
    let state = drive(state, &[idle, idle], 1.5 * LAP_S);
    let standing = state.standings()[0];
    assert_eq!(
        standing.laps_completed, LAPS_PER_RACE,
        "no lap past the last"
    );
    assert_eq!(standing.finish_time_s, finish.finish_time_s);
    assert_eq!(
        standing.best_lap_s, finish.best_lap_s,
        "the best lap is a race lap"
    );
    assert_eq!(
        standing.lap_time_s, finish.lap_time_s,
        "the lap clock stops at the finish"
    );
    assert_eq!(standing.position, 1);
}

#[test]
fn a_finished_kart_takes_and_fires_no_items() {
    // Pole fires whatever it picks up the tick after, so it crosses the
    // line with an empty slot; the bot then laps it over the back
    // straight's boxes.
    let firing: Driver = |state, kart| KartInput {
        use_item: true,
        ..follow_centreline(state, kart)
    };
    let state = started(race(2));
    let mut state = drive_until_finished(state, &[firing, idle], 0);
    assert_eq!(state.kart_readouts()[0].item_slot, ItemSlot::Empty);
    let mut events = Vec::new();
    for _ in 0..ticks(1.5 * LAP_S) {
        let inputs = inputs(&state, &[firing, idle]);
        state = step(state, &inputs, DT);
        events.extend(state.events().iter().copied());
    }
    assert!(
        !events.iter().any(|event| matches!(
            event,
            RaceEvent::ItemPicked { kart: 0, .. } | RaceEvent::ItemFired { kart: 0, .. }
        )),
        "{events:?}"
    );
    assert_eq!(state.kart_readouts()[0].item_slot, ItemSlot::Empty);
}
