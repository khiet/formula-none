//! Bots at the sim's public seam: a race built with `State::with_bots` is
//! stepped with one input per human, and the bot karts drive themselves.
//! Tests read the same transforms and standings a human's kart reports.

use sim::tuning::{
    BOT_LOOKAHEAD_M, BOT_RUBBER_BAND_MAX_FACTOR, BOT_RUBBER_BAND_MIN_FACTOR, BOT_START_HOLD_S,
    COUNTDOWN_S, LAPS_PER_RACE, MINI_TURBO_ORANGE_BOOST_S, TICKS_PER_SECOND, TOP_SPEED_M_PER_S,
};
use sim::{step, KartInput, Phase, State, Surface, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;

/// The race past its countdown, every kart idle on the grid until Go.
fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
    }
    state
}

fn oval() -> Track {
    Track::from_sidecar(OVAL).expect("the oval fixture loads")
}

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

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
    let state = started(State::with_bots(SEED, 1, 1, oval()));
    let grid = state.kart_transforms();
    assert_eq!(grid.len(), 2, "one human and one bot");
    assert_eq!(state.standings().len(), 2);
    assert!(!state.is_bot(0));
    assert!(state.is_bot(1));

    // The bot starts in the other column, so it passes the idle human clean.
    let state = idle_humans(state, 1, 3.0);
    let after = state.kart_transforms();
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
fn a_bot_alone_laps_the_oval_three_times_without_leaving_the_asphalt() {
    let mut state = started(State::with_bots(SEED, 0, 1, oval()));
    let mut lap_times = Vec::new();
    for _ in 0..ticks(LAPS_PER_RACE as f32 * LAP_S) {
        state = step(state, &[], DT);
        let kart = state.kart_transforms()[0];
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

const BRANDS_PATCH: &str =
    include_str!("../../../assets/tracks/brands-patch/brands-patch.track.json");

fn brands_patch() -> Track {
    Track::from_sidecar(BRANDS_PATCH).expect("Brands Patch loads")
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
fn a_bot_drifts_through_each_hairpin_of_brands_patch_on_every_lap() {
    let track = brands_patch();
    let hairpins = hairpins(&track);
    assert_eq!(
        hairpins.len(),
        2,
        "Brands Patch has two hairpins: {hairpins:?}"
    );
    let laps = 2;
    let mut state = started(State::with_bots(SEED, 0, 1, track));
    let mut drifted_in: Vec<Vec<bool>> = vec![vec![false; hairpins.len()]; laps];
    for _ in 0..ticks(laps as f32 * 45.0) {
        state = step(state, &[], DT);
        let standing = state.standings()[0];
        if standing.laps_completed as usize >= laps {
            break;
        }
        if state.kart_transforms()[0].drift.is_some() {
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
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    sidecar["spawn_grid"][1] = serde_json::json!({ "position": position, "yaw": yaw });
    sidecar["boost_pads"] = serde_json::json!([]);
    Track::from_sidecar(&sidecar.to_string()).expect("the moved-slot oval loads")
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
        let bot = state.kart_transforms()[1];
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
        if since_drift_s > MINI_TURBO_ORANGE_BOOST_S + DT && bot.boost_s.is_none() {
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
    let state = started(State::with_bots(SEED, 0, 2, oval()));
    let state = idle_humans(state, 0, 5.0);
    assert_eq!(state.rubber_band(0), 1.0);
    assert_eq!(state.rubber_band(1), 1.0);
}

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`.
fn yaw(rotation: [f32; 4]) -> f32 {
    let [x, y, z, w] = rotation;
    (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z))
}

/// A scripted perfect driver for the oval: the rocket start timed as a bot
/// times it, then full throttle the whole way, steering at a line a little
/// right of the centreline a lookahead ahead (clear of the pipes on the
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
    let transform = state.kart_transforms()[kart];
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

/// The oval without its item boxes: a race of driving alone, with no boost
/// to even the field and no traffic from a boosted kart.
fn oval_without_item_boxes() -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    sidecar["item_boxes"] = serde_json::json!([]);
    Track::from_sidecar(&sidecar.to_string()).expect("the oval without boxes loads")
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
    let mut state = State::with_bots(SEED, 1, 7, oval_without_item_boxes());
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
fn a_scripted_idle_driver_is_last_when_a_bot_finishes_and_the_race_ends() {
    let (order, state) = race_against_seven_bots(None, RACE_S);
    assert_eq!(order[7], 0, "finishing order by grid slot: {order:?}");
    assert_eq!(
        state.phase(),
        Phase::Finished,
        "the race ended within {RACE_S} s"
    );
    let standings = state.standings();
    assert!(
        standings[1..].iter().any(|s| s.finish_time_s.is_some()),
        "a bot finished its laps"
    );
    // The race ends the tuned grace after the first finisher; no bot is so
    // far behind that it is still short of its last lap then.
    assert!(
        standings[1..]
            .iter()
            .all(|s| s.laps_completed >= LAPS_PER_RACE - 1),
        "every bot is on its last lap or finished: {:?}",
        standings
            .iter()
            .map(|s| s.laps_completed)
            .collect::<Vec<_>>()
    );
    assert_eq!(standings[0].laps_completed, 0);
    assert_eq!(standings[0].position, 8);
}

/// The oval with slot 0 moved onto the sand at the apex of the first turn,
/// facing the wall that rings it 12 m outside the centreline, so a bot from
/// that slot drives straight into the wall.
fn oval_with_slot_0_facing_the_first_turns_wall() -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    sidecar["spawn_grid"][0] = serde_json::json!({ "position": [0.0, 0.0, -139.5], "yaw": 0.0 });
    Track::from_sidecar(&sidecar.to_string()).expect("the moved-slot oval loads")
}

#[test]
fn a_bot_held_by_a_wall_backs_up_and_drives_on() {
    let mut state = started(State::with_bots(
        SEED,
        0,
        1,
        oval_with_slot_0_facing_the_first_turns_wall(),
    ));
    let start = state.standings()[0].spline_progress;
    let mut stopped_at = None;
    let mut backed_up = false;
    let mut back_on_the_asphalt = None;
    for tick in 0..ticks(15.0) {
        state = step(state, &[], DT);
        let kart = state.kart_transforms()[0];
        if stopped_at.is_none() && tick > 10 && kart.speed.abs() < 0.01 {
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
    let mut state = started(State::with_bots(SEED, 1, 0, oval()));
    let grid = state.kart_transforms()[0];
    state.hand_to_bot(0);
    assert!(state.is_bot(0));
    assert_eq!(state.human_count(), 1, "the kart is still the human's");

    // The human's idle input is still given, and ignored: the bot drives.
    let state = idle_humans(state, 1, 3.0);
    let driven = state.kart_transforms()[0];
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
    let state = idle_humans(state, 1, 6.0);
    assert!(
        state.kart_transforms()[0].speed.abs() < 0.5,
        "the kart coasted to a stop at {} m/s",
        state.kart_transforms()[0].speed
    );
}

#[test]
fn a_human_handed_to_a_bot_in_the_countdown_sets_up_no_start() {
    // A held throttle before the window stalls a human's start; a bot sits
    // the countdown out, so a kart handed over before Go starts clean.
    let mut handed = State::with_bots(SEED, 1, 0, oval());
    handed.hand_to_bot(0);
    let mut stalled = State::with_bots(SEED, 1, 0, oval());
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
    assert!(handed.kart_transforms()[0].speed > 1.0, "the bot is away");
    assert!(
        stalled.kart_transforms()[0].speed.abs() < 0.01,
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
        let state = started(State::with_bots(seed, 0, 7, oval()));
        let verdicts: Vec<Start> = state
            .kart_transforms()
            .iter()
            .map(|kart| kart.start)
            .collect();
        let again = started(State::with_bots(seed, 0, 7, oval()));
        let repeat: Vec<Start> = again
            .kart_transforms()
            .iter()
            .map(|kart| kart.start)
            .collect();
        assert_eq!(verdicts, repeat, "seed {seed} rolls the same starts twice");
        let state = step(state, &[], DT);
        for (slot, kart) in state.kart_transforms().iter().enumerate() {
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
