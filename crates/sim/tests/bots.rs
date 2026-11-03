//! Bots at the sim's public seam: a race built with `State::with_bots` is
//! stepped with one input per human, and the bot karts drive themselves.
//! Tests read the same transforms and standings a human's kart reports.

mod common;

use common::{
    bell_deer, goldstone, monarch, oval, oval_without_item_boxes, started, ticks, track,
    with_coins_at, with_slot_at, without_coins, without_item_boxes, yaw, DT, OVAL, SEED,
};

use sim::tuning::{
    BOT_LOOKAHEAD_M, BOT_RUBBER_BAND_MAX_FACTOR, BOT_RUBBER_BAND_MIN_FACTOR,
    BOT_STALL_SPEED_M_PER_S, BOT_START_HOLD_S, COUNTDOWN_S, LAPS_PER_RACE, MINI_TURBO_RED_BOOST_S,
    TOP_SPEED_M_PER_S,
};
use sim::{step, BotLevel, KartInput, Phase, State, Surface, Track, Tyres};

/// Step `state` for `seconds` with every human idle.
fn idle_humans(mut state: State, humans: usize, seconds: f32) -> State {
    let inputs = vec![KartInput::default(); humans];
    for _ in 0..ticks(seconds) {
        state = step(state, &inputs, DT);
    }
    state
}

#[test]
fn a_bot_fills_the_slot_behind_the_human_and_drives_off_without_an_input() {
    let state = started(State::with_bots(SEED, 1, 1, oval(), BotLevel::Basic));
    let grid = state.kart_readouts();
    assert_eq!(grid.len(), 2, "one human and one bot");
    assert_eq!(state.standings().len(), 2);
    assert!(!state.is_bot(0));
    assert!(state.is_bot(1));

    // The bot starts in the other column, so it passes the idle human clean.
    let state = idle_humans(state, 1, 3.0);
    let after = state.kart_readouts();
    assert!(
        (after[0].position[0] - grid[0].position[0]).abs() < 0.01
            && (after[0].position[2] - grid[0].position[2]).abs() < 0.01,
        "the idle human stays on the grid: {:?} to {:?}",
        grid[0].position,
        after[0].position
    );
    assert!(
        after[1].position[2] < grid[1].position[2] - 30.0,
        "the bot drove up the front straight from z {} to z {}",
        grid[1].position[2],
        after[1].position[2]
    );
    assert!(
        after[1].speed > 20.0,
        "the bot is at {} m/s",
        after[1].speed
    );
    assert_eq!(
        state.standings()[0].position,
        2,
        "the bot passed the idle human"
    );
}

/// A lap at top speed takes about 15 s; this is enough for one from the grid.
const LAP_S: f32 = 20.0;

#[test]
fn a_bot_alone_laps_the_oval_for_the_race_without_leaving_the_asphalt() {
    // No coins and no item boxes: what the bot takes changes when
    // everything after happens, and with it whether the bot meets its own
    // hazard, which this test does not test.
    let mut state = started(State::with_bots(
        SEED,
        0,
        1,
        track(&without_item_boxes(&without_coins(OVAL))),
        BotLevel::Basic,
    ));
    let mut lap_times = Vec::new();
    for _ in 0..ticks(LAPS_PER_RACE as f32 * LAP_S) {
        state = step(state, &[], DT);
        let kart = state.kart_readouts()[0];
        assert_eq!(
            state.kart_surface(0),
            Surface::Asphalt,
            "the bot left the asphalt at {:?} after {} laps",
            kart.position,
            state.standings()[0].laps_completed
        );
        let standing = state.standings()[0];
        if standing.laps_completed as usize > lap_times.len() {
            lap_times.push(standing.best_lap_s.expect("a lap just completed"));
        }
        if standing.laps_completed == LAPS_PER_RACE {
            break;
        }
    }
    assert_eq!(
        lap_times.len(),
        LAPS_PER_RACE as usize,
        "lap times {lap_times:?}"
    );
    assert!(
        lap_times.iter().all(|&lap| lap < LAP_S),
        "every lap at racing pace: {lap_times:?}"
    );
}

/// The spline progress spans of the track's hairpins: stretches where the
/// centreline turns through more than 150 degrees within 70 m, found from
/// the centreline points alone.
fn hairpins(track: &Track) -> Vec<std::ops::Range<f32>> {
    let lap = track.spline_length();
    let heading = |progress: f32| {
        let a = track.point_at(progress);
        let b = track.point_at(progress + 1.0);
        (b[0] - a[0]).atan2(b[2] - a[2])
    };
    let turned = |from: f32, to: f32| {
        let mut total: f32 = 0.0;
        let mut previous = heading(from);
        let mut at = from + 1.0;
        while at <= to {
            let next = heading(at);
            let mut delta = next - previous;
            if delta > std::f32::consts::PI {
                delta -= 2.0 * std::f32::consts::PI;
            } else if delta < -std::f32::consts::PI {
                delta += 2.0 * std::f32::consts::PI;
            }
            total += delta;
            previous = next;
            at += 1.0;
        }
        total.abs()
    };
    let mut spans = Vec::new();
    let mut at = 0.0;
    while at < lap {
        if turned(at, at + 70.0) > 150.0_f32.to_radians() {
            let end = at + 70.0;
            spans.push(at..end);
            at = end;
        } else {
            at += 5.0;
        }
    }
    spans
}

#[test]
fn a_bot_drifts_through_the_hairpin_of_bell_deer_on_every_lap() {
    let track = bell_deer();
    let hairpins = hairpins(&track);
    assert_eq!(hairpins.len(), 1, "Bell Deer has one hairpin: {hairpins:?}");
    let laps = LAPS_PER_RACE as usize;
    let mut state = started(State::with_bots(SEED, 0, 1, track, BotLevel::Basic));
    let mut drifted_in: Vec<Vec<bool>> = vec![vec![false; hairpins.len()]; laps];
    for _ in 0..ticks(laps as f32 * 75.0) {
        state = step(state, &[], DT);
        let standing = state.standings()[0];
        if standing.laps_completed as usize >= laps {
            break;
        }
        if state.kart_readouts()[0].drift.is_some() {
            for (hairpin, span) in hairpins.iter().enumerate() {
                if span.contains(&standing.spline_progress) {
                    drifted_in[standing.laps_completed as usize][hairpin] = true;
                }
            }
        }
    }
    assert_eq!(state.standings()[0].laps_completed as usize, laps);
    assert!(
        drifted_in.iter().flatten().all(|&drifted| drifted),
        "drifted per lap per hairpin: {drifted_in:?}"
    );
}

/// Step `state` for `seconds` with every human at full throttle, straight.
fn throttle_humans(mut state: State, humans: usize, seconds: f32) -> State {
    let inputs = vec![
        KartInput {
            throttle: 1.0,
            ..KartInput::default()
        };
        humans
    ];
    for _ in 0..ticks(seconds) {
        state = step(state, &inputs, DT);
    }
    state
}

/// The oval with its second grid slot moved onto the back straight, about
/// half a lap short of the line, and its boost pads removed: a bot from that
/// slot starts far behind a kart on the grid and passes it before it laps,
/// and its speed is never a pad's.
fn oval_with_slot_1_on_the_back_straight_and_no_boost_pads() -> Track {
    const BACK_STRAIGHT_PROGRESS_M: f32 = 230.0;
    let oval = oval();
    let position = oval.point_at(BACK_STRAIGHT_PROGRESS_M);
    let [dx, dz] = oval.direction_at(BACK_STRAIGHT_PROGRESS_M);
    // Yaw zero faces -Z and grows counter-clockwise seen from above.
    let yaw = (-dx).atan2(-dz);
    let mut sidecar: serde_json::Value =
        serde_json::from_str(&with_slot_at(OVAL, 1, position, yaw)).expect("valid JSON");
    sidecar["boost_pads"] = serde_json::json!([]);
    track(&sidecar.to_string())
}

#[test]
fn the_rubber_band_hurries_a_bot_far_behind_and_slows_one_far_ahead_within_the_tuned_bounds() {
    // The human idles on the grid; the bot starts over 100 m behind it on
    // the back straight, passes it and runs over 100 m ahead.
    let mut state = started(State::with_bots(
        SEED,
        1,
        1,
        oval_with_slot_1_on_the_back_straight_and_no_boost_pads(),
        BotLevel::Basic,
    ));
    assert_eq!(state.rubber_band(0), 1.0, "a human is never banded");
    assert_eq!(
        state.rubber_band(1),
        BOT_RUBBER_BAND_MAX_FACTOR,
        "far behind at the start"
    );
    let mut least = f32::MAX;
    let mut most = f32::MIN;
    let mut fastest_behind: f32 = 0.0;
    let mut fastest_ahead: f32 = 0.0;
    let mut level_band = None;
    let mut since_drift_s = f32::MAX;
    for _ in 0..ticks(LAP_S) {
        let was_behind = state.standings()[1].position == 2;
        state = step(state, &[KartInput::default()], DT);
        let band = state.rubber_band(1);
        least = least.min(band);
        most = most.max(band);
        let bot = state.kart_readouts()[1];
        let behind = state.standings()[1].position == 2;
        if was_behind && !behind {
            level_band = Some(band);
        }
        // A mini-turbo or a start boost scales the tuned top speed, not
        // the banded one, so a drift and the boost it releases are left
        // out, and so is any boost under way.
        since_drift_s = if bot.drift.is_some() {
            0.0
        } else {
            since_drift_s + DT
        };
        if since_drift_s > MINI_TURBO_RED_BOOST_S + DT && bot.boost_s.is_none() {
            if behind {
                fastest_behind = fastest_behind.max(bot.speed);
            } else {
                fastest_ahead = fastest_ahead.max(bot.speed);
            }
        }
    }
    assert!(
        least >= BOT_RUBBER_BAND_MIN_FACTOR && most <= BOT_RUBBER_BAND_MAX_FACTOR,
        "band ran {least}..{most}"
    );
    assert_eq!(most, BOT_RUBBER_BAND_MAX_FACTOR);
    assert_eq!(
        least, BOT_RUBBER_BAND_MIN_FACTOR,
        "far enough ahead for the full band"
    );
    let level = level_band.expect("the bot passed the human");
    assert!(
        (level - 1.0).abs() < 0.01,
        "level with the human the band is {level}"
    );
    assert!(
        fastest_behind > TOP_SPEED_M_PER_S + 1.0,
        "behind the human the bot runs past top speed: {fastest_behind}"
    );
    assert!(
        fastest_ahead < TOP_SPEED_M_PER_S,
        "ahead of the human the bot never reaches top speed: {fastest_ahead}"
    );

    let state = throttle_humans(state, 1, 1.0);
    assert_eq!(state.rubber_band(0), 1.0);
}

#[test]
fn a_bot_in_a_race_with_no_humans_has_no_rubber_band() {
    let state = started(State::with_bots(SEED, 0, 2, oval(), BotLevel::Basic));
    let state = idle_humans(state, 0, 5.0);
    assert_eq!(state.rubber_band(0), 1.0);
    assert_eq!(state.rubber_band(1), 1.0);
}

/// A scripted perfect driver for the oval: the rocket start timed as a bot
/// times it, then full throttle the whole way, steering at a line a little
/// right of the centreline a lookahead ahead (clear of the tyre walls on the
/// inside of the turns), and hopping into a drift held through each turn
/// so the exit releases a mini-turbo.
fn perfect_driver(state: &State, kart: usize) -> KartInput {
    const RIGHT_OF_CENTRE_M: f32 = 2.5;
    const STEER_GAIN: f32 = 4.0;
    const TURN_CURVATURE: f32 = 0.02;
    if let Phase::Countdown { remaining_s } = state.phase() {
        return KartInput {
            throttle: f32::from(u8::from(remaining_s <= BOT_START_HOLD_S)),
            ..KartInput::default()
        };
    }
    let transform = state.kart_readouts()[kart];
    let progress = state.standings()[kart].spline_progress;
    let track = state.track();
    let ahead = track.point_at(progress + BOT_LOOKAHEAD_M);
    let [tx, tz] = track.direction_at(progress + BOT_LOOKAHEAD_M);
    // Right of the direction of travel (x, z), seen from above, is (-z, x).
    let target = [
        ahead[0] - tz * RIGHT_OF_CENTRE_M,
        ahead[2] + tx * RIGHT_OF_CENTRE_M,
    ];
    let (dx, dz) = (
        target[0] - transform.position[0],
        target[1] - transform.position[2],
    );
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    // Positive steer turns the kart right; right of a yaw is (cos, -sin) in x, z.
    let (sin, cos) = yaw(transform.rotation).sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    let curvature = track.curvature_at(progress + BOT_LOOKAHEAD_M);
    let turning = curvature.abs() > TURN_CURVATURE;
    let mut steer = (rightward * STEER_GAIN).clamp(-1.0, 1.0);
    // The hop fixes the drift's direction from the steer: into the turn.
    if turning && transform.drift.is_none() {
        let inward = if curvature > 0.0 { -1.0 } else { 1.0 };
        steer = inward * steer.abs().max(0.25);
    }
    KartInput {
        steer,
        throttle: 1.0,
        hop: turning,
        ..KartInput::default()
    }
}

/// Race one human driven by `driver`, from the top of the countdown,
/// against seven bots on the oval without its item boxes until the race
/// ends or `seconds` of racing are up, so the race compares the driving;
/// the bots' items are covered in `items.rs`. Returns the finishing order
/// as grid slots, who finished first and then the rest by race position
/// when the run ends, and the state at the end.
fn race_against_seven_bots(
    driver: Option<fn(&State, usize) -> KartInput>,
    seconds: f32,
) -> (Vec<usize>, State) {
    // No boxes: no boost to even the field and no traffic from a boosted kart.
    let mut state = State::with_bots(SEED, 1, 7, oval_without_item_boxes(), BotLevel::Basic);
    let mut finished: Vec<usize> = Vec::new();
    for _ in 0..ticks(seconds + COUNTDOWN_S) {
        let input = driver.map_or(KartInput::default(), |driver| driver(&state, 0));
        state = step(state, &[input], DT);
        let standings = state.standings();
        let mut done: Vec<usize> = (0..standings.len())
            .filter(|&slot| {
                standings[slot].laps_completed >= LAPS_PER_RACE && !finished.contains(&slot)
            })
            .collect();
        // Two finishing on one tick are ordered by race position.
        done.sort_by_key(|&slot| standings[slot].position);
        finished.extend(done);
        if state.phase() == Phase::Finished {
            break;
        }
    }
    let standings = state.standings();
    let mut unfinished: Vec<usize> = (0..standings.len())
        .filter(|slot| !finished.contains(slot))
        .collect();
    unfinished.sort_by_key(|&slot| standings[slot].position);
    finished.extend(unfinished);
    (finished, state)
}

/// Long enough for the whole field to finish, with the traffic off the grid.
const RACE_S: f32 = LAPS_PER_RACE as f32 * 30.0;

#[test]
fn a_scripted_perfect_driver_beats_all_seven_bots() {
    let (order, state) = race_against_seven_bots(Some(perfect_driver), RACE_S);
    assert_eq!(order[0], 0, "finishing order by grid slot: {order:?}");
    assert!(state.standings()[0].laps_completed >= LAPS_PER_RACE);
}

#[test]
fn a_scripted_idle_driver_is_last_behind_all_seven_bots_and_holds_the_race_open() {
    let (order, state) = race_against_seven_bots(None, RACE_S);
    assert_eq!(order[7], 0, "finishing order by grid slot: {order:?}");
    let standings = state.standings();
    assert!(
        standings[1..].iter().all(|s| s.finish_time_s.is_some()),
        "every bot finished its laps"
    );
    assert!(
        matches!(state.phase(), Phase::Racing { .. }),
        "only the human ends the race: {:?}",
        state.phase()
    );
    assert_eq!(standings[0].laps_completed, 0);
    assert_eq!(standings[0].position, 8);
}

/// The oval with slot 0 moved onto the sand at the apex of the first turn,
/// facing the wall that rings it 12 m outside the centreline, so a bot from
/// that slot drives straight into the wall.
fn oval_with_slot_0_facing_the_first_turns_wall() -> Track {
    track(&with_slot_at(OVAL, 0, [0.0, 0.0, -139.5], 0.0))
}

#[test]
fn a_bot_held_by_a_wall_backs_up_and_drives_on() {
    let mut state = started(State::with_bots(
        SEED,
        0,
        1,
        oval_with_slot_0_facing_the_first_turns_wall(),
        BotLevel::Basic,
    ));
    let start = state.standings()[0].spline_progress;
    let mut stopped_at = None;
    let mut backed_up = false;
    let mut back_on_the_asphalt = None;
    for tick in 0..ticks(15.0) {
        state = step(state, &[], DT);
        let kart = state.kart_readouts()[0];
        // The wall holds the kart at its corner to a crawl the bot counts
        // as a stall.
        if stopped_at.is_none() && tick > 10 && kart.speed.abs() < BOT_STALL_SPEED_M_PER_S {
            stopped_at = Some(tick);
        }
        if stopped_at.is_some() && kart.speed < -1.0 {
            backed_up = true;
        }
        if backed_up && state.kart_surface(0) == Surface::Asphalt && kart.speed > 10.0 {
            back_on_the_asphalt = Some(tick);
            break;
        }
    }
    let stopped = stopped_at.expect("the wall stopped the bot");
    assert!(
        backed_up,
        "the bot never backed up after stopping at tick {stopped}"
    );
    let rejoined = back_on_the_asphalt.expect("the bot never got back on the track at speed");
    assert!(
        rejoined < ticks(10.0),
        "rejoined the track at tick {rejoined}, stopped at tick {stopped}"
    );
    let rejoined_at = state.standings()[0].spline_progress;
    assert!(rejoined_at > start, "past where it started");
    let state = idle_humans(state, 0, 1.0);
    assert!(
        state.standings()[0].spline_progress > rejoined_at + 10.0,
        "and is on its way round"
    );
}

#[test]
fn a_humans_kart_handed_to_a_bot_drives_itself_until_handed_back() {
    // The second human, idle on the grid, holds the race open.
    let mut state = started(State::with_bots(SEED, 2, 0, oval(), BotLevel::Basic));
    let grid = state.kart_readouts()[0];
    state.hand_to_bot(0);
    assert!(state.is_bot(0));
    assert_eq!(state.human_count(), 2, "the kart is still the human's");

    // The human's idle input is still given, and ignored: the bot drives.
    let state = idle_humans(state, 2, 3.0);
    let driven = state.kart_readouts()[0];
    assert!(
        driven.position[2] < grid.position[2] - 30.0 && driven.speed > 20.0,
        "the bot drove the kart off the grid: {:?} at {} m/s",
        driven.position,
        driven.speed
    );
    assert_eq!(
        state.rubber_band(0),
        1.0,
        "a human's kart is never banded, whoever drives it"
    );

    // Handed back, the idle human's input holds the kart again: it coasts
    // down rather than driving on.
    let mut state = state;
    state.hand_back(0);
    assert!(!state.is_bot(0));
    let state = idle_humans(state, 2, 6.0);
    assert!(
        state.kart_readouts()[0].speed.abs() < 0.5,
        "the kart coasted to a stop at {} m/s",
        state.kart_readouts()[0].speed
    );
}

#[test]
fn a_human_handed_to_a_bot_in_the_countdown_sets_up_no_start() {
    // A held throttle before the window stalls a human's start; a bot sits
    // the countdown out, so a kart handed over before Go starts clean.
    let mut handed = State::with_bots(SEED, 1, 0, oval(), BotLevel::Basic);
    handed.hand_to_bot(0);
    let mut stalled = State::with_bots(SEED, 1, 0, oval(), BotLevel::Basic);
    let throttle = [KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }];
    while matches!(handed.phase(), Phase::Countdown { .. }) {
        handed = step(handed, &throttle, DT);
        stalled = step(stalled, &throttle, DT);
    }
    for _ in 0..ticks(0.5) {
        handed = step(handed, &throttle, DT);
        stalled = step(stalled, &throttle, DT);
    }
    assert!(handed.kart_readouts()[0].speed > 1.0, "the bot is away");
    assert!(
        stalled.kart_readouts()[0].speed.abs() < 0.01,
        "the human stalled"
    );
}

#[test]
fn bots_roll_the_full_boost_or_a_clean_start_from_the_seed_and_never_stall() {
    use sim::tuning::START_BOOST_FACTOR;
    use sim::Start;
    let mut boosted = 0;
    let mut clean = 0;
    for seed in 0..4 {
        let state = started(State::with_bots(seed, 0, 7, oval(), BotLevel::Basic));
        let verdicts: Vec<Start> = state
            .kart_readouts()
            .iter()
            .map(|kart| kart.start)
            .collect();
        let again = started(State::with_bots(seed, 0, 7, oval(), BotLevel::Basic));
        let repeat: Vec<Start> = again
            .kart_readouts()
            .iter()
            .map(|kart| kart.start)
            .collect();
        assert_eq!(verdicts, repeat, "seed {seed} rolls the same starts twice");
        let state = step(state, &[], DT);
        for (slot, kart) in state.kart_readouts().iter().enumerate() {
            match verdicts[slot] {
                Start::FullBoost => {
                    boosted += 1;
                    assert_eq!(
                        kart.speed,
                        TOP_SPEED_M_PER_S * START_BOOST_FACTOR,
                        "seed {seed} bot {slot} off the line at the boosted top speed"
                    );
                }
                Start::Clean => {
                    clean += 1;
                    assert!(
                        0.0 < kart.speed && kart.speed < 1.0,
                        "seed {seed} bot {slot} crawls off the line: {} m/s",
                        kart.speed
                    );
                }
                other => panic!("seed {seed} bot {slot} rolled {other:?}"),
            }
        }
    }
    assert!(boosted > 0 && clean > 0, "{boosted} boosted, {clean} clean");
}

/// A race of one bot and no humans on the oval, with no coins but those at
/// `coins`, past its countdown.
fn lone_bot_over(coins: impl IntoIterator<Item = [f32; 3]>) -> State {
    let sidecar = with_coins_at(&without_coins(OVAL), coins);
    started(State::with_bots(
        SEED,
        0,
        1,
        track(&sidecar),
        BotLevel::Basic,
    ))
}

#[test]
fn a_bot_that_drives_over_a_coin_carries_it() {
    // A row abreast across the front straight 60 m past the line: wherever
    // the bot drives, its centre crosses one.
    let row = (0..6).map(|k| [25.0 + 2.0 * k as f32, 0.0, -60.0]);
    let mut state = lone_bot_over(row);
    let mut picked = false;
    for _ in 0..ticks(5.0) {
        state = step(state, &[], DT);
        picked |= state
            .events()
            .iter()
            .any(|event| matches!(event, sim::RaceEvent::CoinPicked { kart: 0, .. }));
    }

    assert!(picked, "the bot took a coin");
    assert!(state.kart_readouts()[0].coins >= 1);
}

#[test]
fn a_bots_steering_is_unchanged_by_a_coin_off_its_line() {
    // Left of the front straight's centreline, while the bot keeps right.
    let beside = [26.0, 0.0, -40.0];
    let mut bare = lone_bot_over([]);
    let mut beside_its_line = lone_bot_over([beside]);
    for _ in 0..ticks(6.0) {
        bare = step(bare, &[], DT);
        beside_its_line = step(beside_its_line, &[], DT);
        let (kart, other) = (bare.kart_readouts()[0], beside_its_line.kart_readouts()[0]);
        assert_eq!(other.coins, 0, "the coin is off the bot's line");
        assert_eq!(
            (kart.position, kart.rotation),
            (other.position, other.rotation)
        );
    }
    assert!(
        bare.kart_readouts()[0].position[2] < beside[2],
        "the bot drove past the coin"
    );
}

/// The longest any bot in an eight-bot race on `track` from `seed` sat under
/// its stall speed before it finished, as `(seconds, slot, race clock)`.
/// A spun-out kart is dead to every control and a bot held back for the
/// runaway wheel stands by choice, so neither counts.
fn longest_stall(seed: u64, track: Track) -> (f32, usize, f32) {
    let mut state = started(State::with_bots(seed, 0, 8, track, BotLevel::Basic));
    let mut stalled_s = [0.0; 8];
    let mut longest = (0.0, 0, 0.0);
    for _ in 0..ticks(LAPS_PER_RACE as f32 * 120.0) {
        state = step(state, &[], DT);
        let Phase::Racing { elapsed_s, .. } = state.phase() else {
            break;
        };
        let standings = state.standings();
        if standings.iter().all(|s| s.laps_completed >= LAPS_PER_RACE) {
            break;
        }
        for (slot, kart) in state.kart_readouts().iter().enumerate() {
            let finished = standings[slot].laps_completed >= LAPS_PER_RACE;
            if finished || kart.speed.abs() >= BOT_STALL_SPEED_M_PER_S {
                stalled_s[slot] = 0.0;
            } else if kart.spin_out_s.is_none() && !state.bot_held_for_the_wheel(slot) {
                stalled_s[slot] += DT;
                if stalled_s[slot] > longest.0 {
                    longest = (stalled_s[slot], slot, elapsed_s);
                }
            }
        }
    }
    longest
}

/// Each bot's finishing time in a race of eight bots on `course` from
/// `seed`, `None` for one still racing after `seconds`.
fn eight_bots_finishing_times(seed: u64, course: Track, seconds: f32) -> Vec<Option<f32>> {
    let mut state = started(State::with_bots(seed, 0, 8, course, BotLevel::Basic));
    for _ in 0..ticks(seconds) {
        state = step(state, &[], DT);
        if state.standings().iter().all(|s| s.finish_time_s.is_some()) {
            break;
        }
    }
    state.standings().iter().map(|s| s.finish_time_s).collect()
}

#[test]
fn eight_bots_race_the_laps_of_monarch_and_every_one_finishes() {
    let times = eight_bots_finishing_times(SEED, monarch(), LAPS_PER_RACE as f32 * 90.0);
    assert!(
        times.iter().all(Option::is_some),
        "finishing times: {times:?}"
    );
}

#[test]
fn eight_bots_race_the_laps_of_goldstone_and_every_one_finishes() {
    let times = eight_bots_finishing_times(SEED, goldstone(), LAPS_PER_RACE as f32 * 90.0);
    assert!(
        times.iter().all(Option::is_some),
        "finishing times: {times:?}"
    );
}

/// The slots still racing when a race of eight bots at `level` on `course`
/// from `seed` ended: the end cap fired before they finished.
fn non_finishers(seed: u64, course: Track, level: BotLevel, tyres: Tyres) -> Vec<usize> {
    let mut state = started(State::with_tyres(seed, 0, 8, course, level, tyres));
    while matches!(state.phase(), Phase::Racing { .. }) {
        state = step(state, &[], DT);
    }
    let standings = state.standings();
    (0..standings.len())
        .filter(|&slot| standings[slot].finish_time_s.is_none())
        .collect()
}

/// A course by name, built afresh for each race.
type Course = (&'static str, fn() -> Track);

/// Every course a sweep races.
const COURSES: [Course; 4] = [
    ("oval", oval),
    ("Bell Deer", bell_deer),
    ("Goldstone", goldstone),
    ("Monarch", monarch),
];

/// A kart that hits Monarch's close walls at boost speed can fly over one
/// and land back inside; a checkpoint it passes in the air still has to
/// count, or it ends the race a lap short. Too slow for a debug build; run it
/// with `cargo test --release -p sim --test bots -- --ignored` (a few minutes,
/// with the stuck sweep).
#[test]
#[ignore]
fn in_200_races_of_eight_bots_on_monarch_every_one_finishes_at_every_level() {
    for level in [BotLevel::Basic, BotLevel::Pro, BotLevel::Ace] {
        let races: Vec<_> = (0..200)
            .map(|seed| {
                std::thread::spawn(move || {
                    (seed, non_finishers(seed, monarch(), level, Tyres::Off))
                })
            })
            .collect();
        for race in races {
            let (seed, slots) = race.join().expect("the race ran");
            assert!(
                slots.is_empty(),
                "{level:?} seed {seed}: karts {slots:?} never finished"
            );
        }
    }
}

/// A bot wears out as a human does, and on worn tyres it must still get round
/// (ADR 0080). Too slow for a debug build; run it with the sweep above.
#[test]
#[ignore]
fn in_200_races_of_eight_bots_on_tyres_on_every_one_finishes_every_course_at_every_level() {
    for (name, course) in COURSES {
        for level in [BotLevel::Basic, BotLevel::Pro, BotLevel::Ace] {
            let races: Vec<_> = (0..200)
                .map(|seed| {
                    std::thread::spawn(move || {
                        (seed, non_finishers(seed, course(), level, Tyres::On))
                    })
                })
                .collect();
            for race in races {
                let (seed, slots) = race.join().expect("the race ran");
                assert!(
                    slots.is_empty(),
                    "{name} {level:?} seed {seed}: karts {slots:?} never finished"
                );
            }
        }
    }
}

/// Under its stall speed this long, a bot is stuck.
const STUCK_S: f32 = 3.0;

#[test]
fn a_bot_jammed_short_of_the_wheels_crossing_backs_out() {
    // On 38 a bot crashed to a stop 90 m short of the crossing just before
    // a pass, and jittered there in and out of waiting for the wheel until
    // the pass was over.
    let (seconds, slot, at_s) = longest_stall(38, bell_deer());
    assert!(
        seconds <= STUCK_S,
        "bot {slot} stuck {seconds} s until {at_s} s"
    );
}

#[test]
fn a_bot_waiting_for_the_wheel_never_backs_up() {
    // On each of these a bot waiting for a pass held the brake, which backed
    // it up into whatever was behind, pinned there through the pass.
    // Rolling back as it starts to wait, from backing out of a stall, it
    // only slows.
    for seed in [51, 121, 152, 163] {
        let mut state = started(State::with_bots(seed, 0, 8, bell_deer(), BotLevel::Basic));
        let mut waited = false;
        for _ in 0..ticks(LAPS_PER_RACE as f32 * 90.0) {
            let before = state.kart_readouts();
            state = step(state, &[], DT);
            for (slot, kart) in state.kart_readouts().iter().enumerate() {
                if state.bot_held_for_the_wheel(slot) && kart.spin_out_s.is_none() {
                    waited = true;
                    assert!(
                        kart.speed >= before[slot].speed.min(0.0),
                        "seed {seed}: bot {slot} backs up from {} to {} m/s at {:?}",
                        before[slot].speed,
                        kart.speed,
                        state.phase()
                    );
                }
            }
            if !matches!(state.phase(), Phase::Racing { .. }) {
                break;
            }
        }
        assert!(waited, "seed {seed}: no bot waited for the wheel");
    }
}

/// Every course's 200-seed sweep, too slow for a debug build; run it with
/// `cargo test --release -p sim --test bots -- --ignored` (about 30 s) after
/// changing how a bot drives.
#[test]
#[ignore]
fn in_200_races_of_eight_bots_on_each_course_no_bot_is_ever_stuck() {
    for (name, course) in COURSES {
        let races: Vec<_> = (0..200)
            .map(|seed| std::thread::spawn(move || (seed, longest_stall(seed, course()))))
            .collect();
        for race in races {
            let (seed, (seconds, slot, at_s)) = race.join().expect("the race ran");
            assert!(
                seconds <= STUCK_S,
                "{name} seed {seed}: bot {slot} stuck {seconds} s until {at_s} s"
            );
        }
    }
}

#[test]
fn an_ace_bot_ahead_of_every_human_is_never_slowed_and_is_still_hurried_from_behind() {
    // The same start as the Basic band's test: the bot over 100 m behind
    // the idle human, passing it and running over 100 m ahead.
    let mut state = started(State::with_bots(
        SEED,
        1,
        1,
        oval_with_slot_1_on_the_back_straight_and_no_boost_pads(),
        BotLevel::Ace,
    ));
    assert_eq!(
        state.rubber_band(1),
        BOT_RUBBER_BAND_MAX_FACTOR,
        "far behind at the start the band is the same at every level"
    );
    let mut least = f32::MAX;
    let mut passed = false;
    for _ in 0..ticks(LAP_S) {
        state = step(state, &[KartInput::default()], DT);
        least = least.min(state.rubber_band(1));
        passed |= state.standings()[1].position == 1;
    }
    assert!(passed, "the bot passed the human");
    assert_eq!(
        least, 1.0,
        "ahead of the human an Ace bot runs at full speed"
    );
}

#[test]
fn ace_bots_always_roll_the_full_boost_at_go() {
    use sim::Start;
    for seed in 0..4 {
        let state = started(State::with_bots(seed, 0, 7, oval(), BotLevel::Ace));
        let verdicts: Vec<Start> = state
            .kart_readouts()
            .iter()
            .map(|kart| kart.start)
            .collect();
        assert_eq!(verdicts, vec![Start::FullBoost; 7], "seed {seed}");
    }
}
