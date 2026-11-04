//! Lap counting and race position at the sim's public seam: drive karts on
//! the oval fixture and read `standings`. The oval's front straight runs
//! along -Z from the start line at z = 0 through its first checkpoints, with
//! the grid a few metres behind the line (layouts/oval.py). Both tracks are
//! walled along both edges of the run-off, so a kart can leave the asphalt
//! but never the course (ADR 0022); Bell Deer joins in for the cut
//! its walls close and the bridge its parapets keep a kart on.

mod common;

use common::{
    bell_deer, follow_centreline, monarch, oval, race, run, started, steer_toward, straight,
    throttle, ticks, track, with_slot_at, without_wheel_crossings, BELL_DEER, DT, MONARCH, OVAL,
    SEED,
};

use sim::tuning::DEFAULT_LAPS;
use sim::{step, BotLevel, KartInput, Standing, State, Surface, Track};

/// The hairpin on Bell Deer (layouts/bell_deer.py): a 180 degree
/// left-hander of 17 m radius, its exit 34 m to the left of its entry
/// across the infield. The infield is walled along the inside edge of the
/// run-off, 12 m left of the centreline, so 5 m from the hairpin's centre
/// round the turn. The checkpoint 609 m into the lap, the eighth, is the
/// last before it.
const HAIRPIN_ENTRY_M: f32 = 652.9;
const HAIRPIN_EXIT_M: f32 = 706.3;
const CHECKPOINT_BEFORE_THE_HAIRPIN: usize = 7;
/// Where the back straight crosses over the underpass, 5 m up, with a wall
/// standing on the deck 12 m either side of the centreline.
const BRIDGE_DECK_PROGRESS_M: f32 = 1020.7;
const DECK_HEIGHT_M: f32 = 5.0;
const RUN_OFF_HALF_WIDTH_M: f32 = 12.0;

fn only_standing(state: &State) -> Standing {
    let standings = state.standings();
    assert_eq!(standings.len(), 1);
    standings[0]
}

#[test]
fn a_kart_on_the_grid_is_on_lap_one_at_the_start_line_in_first_place() {
    let standing = only_standing(&race(1));

    assert_eq!(standing.position, 1);
    assert_eq!(standing.lap, 1);
    assert_eq!(standing.laps_completed, 0);
    assert_eq!(standing.checkpoint, 0);
    assert_eq!(standing.lap_time_s, 0.0);
    assert_eq!(standing.best_lap_s, None);
    // The grid sits just behind the line: the far end of the lap.
    let lap = oval().spline_length();
    assert!(
        standing.spline_progress > lap - 30.0 && standing.spline_progress < lap,
        "progress {} on a {lap} m lap",
        standing.spline_progress
    );
}

/// What steers a kart in a multi-kart test: reads the state and the kart's
/// grid slot, returns its input for the tick.
type Driver = fn(&State, usize) -> KartInput;

/// Step every kart in `drivers` by its own driver for `seconds`; a `None`
/// driver leaves its kart idle.
fn drive(mut state: State, drivers: &[Option<Driver>], seconds: f32) -> State {
    for _ in 0..ticks(seconds) {
        let inputs: Vec<KartInput> = drivers
            .iter()
            .enumerate()
            .map(|(kart, driver)| {
                driver.map_or(KartInput::default(), |driver| driver(&state, kart))
            })
            .collect();
        state = step(state, &inputs, DT);
    }
    state
}

/// A lap at top speed takes about 15 s; this is enough for one from the grid.
const LAP_S: f32 = 20.0;

#[test]
fn driving_up_the_front_straight_crosses_the_checkpoints_in_order() {
    let state = race(1);
    let lines: Vec<f32> = state.track().checkpoints()[..3]
        .iter()
        .map(|checkpoint| checkpoint.position[2])
        .collect();
    let mut crossed = vec![only_standing(&state).checkpoint];
    let mut state = state;
    for _ in 0..ticks(5.0) {
        state = step(state, &[throttle()], DT);
        let standing = only_standing(&state);
        if crossed.last() != Some(&standing.checkpoint) {
            let z = state.kart_readouts()[0].position[2];
            // The checkpoint was crossed this tick, so the kart is just past it.
            assert!(
                z <= lines[standing.checkpoint] && z > lines[standing.checkpoint] - 1.0,
                "checkpoint {} at z {} crossed with the kart at z {z}",
                standing.checkpoint,
                lines[standing.checkpoint]
            );
            crossed.push(standing.checkpoint);
        }
    }
    assert_eq!(crossed, [0, 1, 2]);
    assert_eq!(only_standing(&state).laps_completed, 0);
}

/// The oval with checkpoint 1 narrowed to a metre either side of the
/// centreline, so a kart driving straight from slot 0, two metres left of
/// it, misses it.
fn oval_with_a_narrow_first_checkpoint() -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    sidecar["checkpoints"][1]["half_width"] = serde_json::json!(1.0);
    Track::from_sidecar(&sidecar.to_string()).expect("the narrowed oval loads")
}

#[test]
fn a_checkpoint_passed_outside_its_width_is_not_crossed_and_later_ones_wait_for_it() {
    let state = started(State::new(SEED, 1, oval_with_a_narrow_first_checkpoint()));
    let state = run(state, std::iter::repeat_n(throttle(), ticks(5.0)));

    let z = state.kart_readouts()[0].position[2];
    assert!(z < -80.0, "the kart is past checkpoints 1 and 2 at z {z}");
    assert_eq!(only_standing(&state).checkpoint, 0);
}

/// A kart that hits a wall at boost speed can ride up it and pass the next
/// checkpoint above it (ADR 0079): this one drives along the top of Monarch's
/// left wall through checkpoint 1, before the respawn takes a kart behind a
/// wall back.
#[test]
fn a_kart_over_a_wall_crosses_the_checkpoint_beside_it() {
    // Monarch's walls stand 8 m from the centreline and are 0.5 m thick.
    const LEFT_OF_CENTRE_M: f32 = 8.25;
    const SHORT_OF_THE_LINE_M: f32 = 2.0;
    const ABOVE_THE_WALL_M: f32 = 1.5;
    let checkpoint = monarch().checkpoints()[1];
    let (sin, cos) = checkpoint.yaw.sin_cos();
    let (travel, right) = ([-sin, -cos], [cos, -sin]);
    let position = [
        checkpoint.position[0] - right[0] * LEFT_OF_CENTRE_M - travel[0] * SHORT_OF_THE_LINE_M,
        checkpoint.position[1] + ABOVE_THE_WALL_M,
        checkpoint.position[2] - right[1] * LEFT_OF_CENTRE_M - travel[1] * SHORT_OF_THE_LINE_M,
    ];
    let on_the_wall = with_slot_at(MONARCH, 0, position, checkpoint.yaw);
    let state = started(State::new(SEED, 1, track(&on_the_wall)));

    let state = run(state, std::iter::repeat_n(throttle(), ticks(1.0)));

    assert_eq!(only_standing(&state).checkpoint, 1);
}

#[test]
fn a_lap_completes_at_the_start_line_after_every_checkpoint_and_records_its_time() {
    let state = drive(race(1), &[Some(follow_centreline)], LAP_S);

    let standing = only_standing(&state);
    assert_eq!(standing.laps_completed, 1);
    assert_eq!(standing.lap, 2);
    assert!(
        (1..4).contains(&standing.checkpoint),
        "a few checkpoints into the second lap, not {}",
        standing.checkpoint
    );
    let best = standing.best_lap_s.expect("a lap was completed");
    assert!((14.0..LAP_S).contains(&best), "best lap {best} s");
    assert!(
        standing.lap_time_s < LAP_S - best,
        "the new lap's clock started at zero"
    );
    assert_eq!(state.kart_surface(0), sim::Surface::Asphalt);
}

#[test]
fn bell_deer_counts_its_checkpoints_the_same_way() {
    let track = bell_deer();
    let lap = track.spline_length();
    let state = started(State::new(SEED, 2, track));
    assert!(
        state.standings()[0].spline_progress > lap - 30.0,
        "the grid is behind the line"
    );

    // Pole drives up the start straight and into the first curve past the
    // first checkpoint while the kart behind stays on the grid.
    let state = drive(state, &[Some(follow_centreline), None], 5.0);
    let standings = state.standings();
    assert_eq!(standings[0].checkpoint, 1);
    assert_eq!(standings[0].laps_completed, 0);
    assert_eq!(standings[1].checkpoint, 0);
    assert_eq!(
        standings.iter().map(|s| s.position).collect::<Vec<_>>(),
        [1, 2]
    );
}

/// `input` with the throttle let off and the brake on while the kart is
/// faster than `cap`.
fn held_to(input: KartInput, speed: f32, cap: f32) -> KartInput {
    if speed > cap {
        KartInput {
            throttle: 0.0,
            brake: 1.0,
            ..input
        }
    } else {
        input
    }
}

/// Full throttle toward `target` (x, z) for ten seconds, long enough for a
/// wall to bring the kart to rest, showing `watch` the state after every
/// tick.
fn drive_toward(mut state: State, target: [f32; 2], mut watch: impl FnMut(&State)) -> State {
    for _ in 0..ticks(10.0) {
        let input = steer_toward(&state, 0, target);
        state = step(state, &[input], DT);
        watch(&state);
    }
    state
}

/// Where the kart is, seen from above.
fn kart_xz(state: &State) -> [f32; 2] {
    let position = state.kart_readouts()[0].position;
    [position[0], position[2]]
}

/// Metres the kart's centre lies left of the centreline, measured square to
/// it at the kart's own spline progress.
fn left_of_centreline(state: &State) -> f32 {
    let progress = only_standing(state).spline_progress;
    let point = state.track().point_at(progress);
    let [dx, dz] = state.track().direction_at(progress);
    let [x, z] = kart_xz(state);
    // Left of the direction of travel (dx, dz), seen from above, is (dz, -dx).
    (x - point[0]) * dz - (z - point[2]) * dx
}

/// A kart a wall has stopped: against it, on the sand, going nowhere under
/// full throttle. `wall_distance` is how far the kart's centre is from the
/// wall's face; a kart is 1.2 m wide and 1.8 m long.
fn assert_held_by_the_wall(state: &State, wall_distance: f32) {
    const HELD_SPEED_M_PER_S: f32 = 0.5;
    const AGAINST_M: f32 = 1.5;
    let kart = state.kart_readouts()[0];
    assert!(
        wall_distance < AGAINST_M,
        "never reached the wall: {wall_distance} m from it at {:?}",
        kart.position
    );
    assert!(
        kart.speed.abs() < HELD_SPEED_M_PER_S,
        "still moving at {} m/s against the wall",
        kart.speed
    );
    assert_eq!(
        state.kart_surface(0),
        Surface::Sand,
        "pinned to the wall on the sand"
    );
}

/// The oval's front straight runs along -Z with its centreline at x = 30
/// and a wall 12 m either side of it (layouts/oval.py).
const OVAL_FRONT_STRAIGHT_X: f32 = 30.0;
const OVAL_WALL_X: f32 = OVAL_FRONT_STRAIGHT_X + 12.0;

#[test]
fn steering_off_the_ovals_front_straight_ends_against_the_wall_at_the_run_offs_edge() {
    let state = started(State::new(SEED, 1, oval()));
    // Hard right off the grid toward a point well beyond the run-off.
    let target = [OVAL_WALL_X + 30.0, kart_xz(&state)[1] - 20.0];
    let state = drive_toward(state, target, |state| {
        let [x, _] = kart_xz(state);
        assert!(x < OVAL_WALL_X + 0.1, "went through the wall at x {x}");
    });

    assert_held_by_the_wall(&state, OVAL_WALL_X - kart_xz(&state)[0]);
    assert_eq!(only_standing(&state).checkpoint, 0);
}

#[test]
fn cutting_the_hairpin_across_the_infield_ends_against_the_inside_wall_short_of_the_exit() {
    // The infield is short, so the kart turns in slow enough to meet its
    // wall square on rather than slide round its end to the exit.
    const TURN_IN_SPEED_M_PER_S: f32 = 8.0;
    const SHORT_OF_THE_ENTRY_M: f32 = 8.0;
    let state = round_to(
        started(State::new(
            SEED,
            1,
            track(&without_wheel_crossings(BELL_DEER)),
        )),
        HAIRPIN_ENTRY_M - SHORT_OF_THE_ENTRY_M,
        HAIRPIN_ENTRY_M - 60.0,
        TURN_IN_SPEED_M_PER_S,
    );
    assert_eq!(
        only_standing(&state).checkpoint,
        CHECKPOINT_BEFORE_THE_HAIRPIN
    );
    let inside_wall = |state: &State| RUN_OFF_HALF_WIDTH_M - left_of_centreline(state);

    // Straight across the infield toward the exit straight, at the point
    // abreast of the kart.
    let exit = state
        .track()
        .point_at(HAIRPIN_EXIT_M + SHORT_OF_THE_ENTRY_M);
    let state = drive_toward(state, [exit[0], exit[2]], |state| {
        let progress = only_standing(state).spline_progress;
        assert!(
            progress < HAIRPIN_EXIT_M,
            "reached the exit at {progress} m"
        );
        assert!(
            inside_wall(state) > -0.1,
            "went through the wall at {:?}",
            kart_xz(state)
        );
    });

    assert_held_by_the_wall(&state, inside_wall(&state));
    assert_eq!(
        only_standing(&state).checkpoint,
        CHECKPOINT_BEFORE_THE_HAIRPIN
    );
}

/// Follow the centreline until the kart's spline progress reaches `progress_m`
/// on its first lap, holding its speed to `cap` from `cap_from_m` on; gives
/// up after a lap's worth of time. The grid is behind the line, so the
/// kart's progress starts near the lap's end: only a reading just past
/// `progress_m` counts as reaching it.
fn round_to(mut state: State, progress_m: f32, cap_from_m: f32, cap: f32) -> State {
    const JUST_PAST_M: f32 = 30.0;
    for _ in 0..ticks(90.0) {
        let speed = state.kart_readouts()[0].speed;
        let progress = only_standing(&state).spline_progress;
        let input = follow_centreline(&state, 0);
        let input = if (cap_from_m..progress_m).contains(&progress) {
            held_to(input, speed, cap)
        } else {
            input
        };
        state = step(state, &[input], DT);
        let progress = only_standing(&state).spline_progress;
        if (progress_m..progress_m + JUST_PAST_M).contains(&progress) {
            return state;
        }
    }
    panic!("never reached {progress_m} m along the lap");
}

#[test]
fn steering_off_the_deck_ends_against_its_wall_still_on_the_bridge() {
    // Slow enough to turn square to the deck within its run-off.
    const LEAVE_SPEED_M_PER_S: f32 = 10.0;
    let state = round_to(
        started(State::new(
            SEED,
            1,
            track(&without_wheel_crossings(BELL_DEER)),
        )),
        BRIDGE_DECK_PROGRESS_M - 6.0,
        BRIDGE_DECK_PROGRESS_M - 60.0,
        LEAVE_SPEED_M_PER_S,
    );
    let checkpoint = only_standing(&state).checkpoint;
    let height = |state: &State| state.kart_readouts()[0].position[1];
    assert!(height(&state) > DECK_HEIGHT_M - 1.0, "on the deck");
    let outside_wall = |state: &State| RUN_OFF_HALF_WIDTH_M + left_of_centreline(state);

    // Hard right, square off the deck toward the underpass below.
    let progress = only_standing(&state).spline_progress;
    let point = state.track().point_at(progress);
    let [dx, dz] = state.track().direction_at(progress);
    let target = [point[0] - dz * 40.0, point[2] + dx * 40.0];
    let state = drive_toward(state, target, |state| {
        assert!(
            outside_wall(state) > -0.1,
            "went through the wall at {:?}",
            kart_xz(state)
        );
        assert!(
            height(state) > DECK_HEIGHT_M - 1.5,
            "fell off the bridge at {:?}",
            state.kart_readouts()[0].position
        );
    });

    assert_held_by_the_wall(&state, outside_wall(&state));
    assert_eq!(only_standing(&state).checkpoint, checkpoint);
}

#[test]
fn a_kart_that_misses_a_checkpoint_and_drives_on_is_not_respawned() {
    // The oval's first checkpoint narrowed so the centreline follower, 2.5 m
    // right of the line, misses it; a respawn would move the kart in one
    // tick, and nothing else does.
    const TELEPORT_M: f32 = 2.0;
    let mut state = started(State::new(SEED, 1, oval_with_a_narrow_first_checkpoint()));
    let mut previous = state.kart_readouts()[0].position;
    let mut furthest = 0.0_f32;
    for _ in 0..ticks(LAP_S) {
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
        let position = state.kart_readouts()[0].position;
        let moved = (position[0] - previous[0]).hypot(position[2] - previous[2]);
        assert!(moved < TELEPORT_M, "moved {moved} m in one tick: respawned");
        previous = position;
        furthest = furthest.max(only_standing(&state).spline_progress);
        assert_eq!(only_standing(&state).checkpoint, 0);
    }

    let standing = only_standing(&state);
    assert_eq!(standing.laps_completed, 0);
    assert!(
        furthest > state.track().spline_length() - 30.0,
        "drove the whole lap round without the checkpoint: reached {furthest} m"
    );
}

#[test]
fn spline_progress_climbs_the_whole_way_round_the_oval_and_wraps_at_the_line() {
    let mut state = race(1);
    let lap = state.track().spline_length();
    let mut previous = only_standing(&state).spline_progress;
    let mut wraps = 0;
    for _ in 0..ticks(LAP_S) {
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
        let standing = only_standing(&state);
        let progress = standing.spline_progress;
        assert!(
            (0.0..lap).contains(&progress),
            "progress {progress} on a {lap} m lap"
        );
        assert!(
            !standing.wrong_way,
            "a kart lapping forward is never going the wrong way, even at the line"
        );
        if progress < previous {
            wraps += 1;
            assert!(
                previous > lap - 2.0 && progress < 2.0,
                "progress fell from {previous} to {progress} away from the start line"
            );
        }
        previous = progress;
    }
    assert_eq!(wraps, 2, "the grid is behind the line, then one lap");
}

#[test]
fn race_position_orders_laps_then_checkpoints_then_progress() {
    // Four karts released together. In the right column the front kart
    // laps and the one behind it drives straight for a second and coasts
    // to a stop just past the line; in the left column the front kart
    // drives straight for two seconds and stops past a checkpoint, and the
    // one behind it never moves. Each keeps to its own column, so no kart
    // grazes another: a touch would amplify the last-bit differences
    // between platforms' trigonometry in the test's steering into a
    // different finishing order. The run ends as the lap completes, with
    // the lapping kart short of the stopped one ahead of it in its column.
    let mut state = race(4);
    for second in 0..2 {
        let drivers: [Option<Driver>; 4] = [
            Some(straight),
            Some(follow_centreline),
            None,
            if second < 1 { Some(straight) } else { None },
        ];
        state = drive(state, &drivers, 1.0);
    }
    for _ in 0..ticks(LAP_S) {
        let input = follow_centreline(&state, 1);
        state = step(
            state,
            &[
                KartInput::default(),
                input,
                KartInput::default(),
                KartInput::default(),
            ],
            DT,
        );
        if state.standings()[1].laps_completed == 1 {
            break;
        }
    }

    let standings = state.standings();
    let positions: Vec<u32> = standings.iter().map(|standing| standing.position).collect();
    assert_eq!(positions, [2, 1, 4, 3]);
    assert_eq!(standings[1].laps_completed, 1);
    assert_eq!(standings[0].laps_completed, 0);
    assert!(standings[0].checkpoint > standings[3].checkpoint);
    assert_eq!(standings[2].checkpoint, standings[3].checkpoint);
    // The last two are past the same checkpoint; progress splits them,
    // with the one still on the grid a little behind the line and so last.
    assert!(standings[2].spline_progress > standings[3].spline_progress);
    assert!(standings[3].spline_progress < 10.0, "just past the line");
}

#[test]
fn an_overtake_registers_the_tick_it_happens() {
    // Pole idles while the kart behind it on the other column drives past.
    let mut state = race(2);
    let mut swapped_at = None;
    for tick in 0..ticks(3.0) {
        state = step(state, &[KartInput::default(), throttle()], DT);
        let standings = state.standings();
        if standings[1].position == 1 {
            swapped_at = Some(tick);
            break;
        }
    }
    let tick = swapped_at.expect("the driven kart gets past the idle one");
    let [idle, driven] = state.kart_readouts()[..] else {
        unreachable!("two karts")
    };
    assert!(
        driven.position[2] < idle.position[2],
        "swapped at tick {tick} with the driven kart at z {} and the idle at z {}",
        driven.position[2],
        idle.position[2]
    );
    assert_eq!(
        state
            .standings()
            .iter()
            .map(|s| s.position)
            .collect::<Vec<_>>(),
        [2, 1]
    );
}

/// The oval with its second grid slot moved level with pole, so two karts
/// start with the same spline progress.
fn oval_with_a_level_front_row() -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    let pole_z = sidecar["spawn_grid"][0]["position"][2].clone();
    sidecar["spawn_grid"][1]["position"][2] = pole_z;
    Track::from_sidecar(&sidecar.to_string()).expect("the levelled oval loads")
}

#[test]
fn a_dead_heat_keeps_grid_order() {
    let state = started(State::new(SEED, 2, oval_with_a_level_front_row()));
    let standings = state.standings();
    assert_eq!(standings[0].spline_progress, standings[1].spline_progress);
    assert_eq!(standings[0].position, 1);
    assert_eq!(standings[1].position, 2);

    // Still level after driving straight side by side, still in grid order.
    let state = drive(state, &[Some(straight), Some(straight)], 2.0);
    let standings = state.standings();
    assert_eq!(standings[0].spline_progress, standings[1].spline_progress);
    assert!(
        standings[0].spline_progress > 10.0,
        "both drove past the line"
    );
    assert_eq!(standings[0].position, 1);
    assert_eq!(standings[1].position, 2);
}

#[test]
fn the_races_laps_of_the_oval_end_with_the_lap_counter_on_the_last_and_a_best_lap() {
    let mut state = race(1);
    let mut lap_times = Vec::new();
    let mut laps_seen = 0;
    for _ in 0..ticks(DEFAULT_LAPS as f32 * LAP_S) {
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
        let standing = only_standing(&state);
        if standing.laps_completed > laps_seen {
            laps_seen = standing.laps_completed;
            lap_times.push(standing.best_lap_s.expect("a lap just completed"));
        }
        if laps_seen == DEFAULT_LAPS {
            break;
        }
    }

    let standing = only_standing(&state);
    assert_eq!(standing.laps_completed, DEFAULT_LAPS);
    assert_eq!(
        standing.lap, DEFAULT_LAPS,
        "the counter stays on the last lap"
    );
    assert_eq!(lap_times.len(), DEFAULT_LAPS as usize);
    // The first lap starts from a standstill, so a flying lap is the best.
    let best = standing.best_lap_s.expect("the race's laps were completed");
    assert!(
        best < lap_times[0],
        "best {best} s, first lap {} s",
        lap_times[0]
    );
    assert!(
        lap_times.windows(2).all(|pair| pair[1] <= pair[0]),
        "{lap_times:?}"
    );
}

/// The best lap of the one bot in a race with a human idle on the grid,
/// from the oval's seed at `level`, within forty seconds: with the human
/// never moving the bot is ahead of it the whole way, so the level's
/// rubber band, corner speed and start all tell.
fn bot_best_lap_at(level: BotLevel) -> f32 {
    let mut state = started(State::with_bots(SEED, 1, 1, oval(), level));
    for _ in 0..ticks(2.0 * LAP_S) {
        state = step(state, &[KartInput::default()], DT);
    }
    state.standings()[1]
        .best_lap_s
        .expect("the bot laps the oval twice in forty seconds")
}

#[test]
fn from_one_seed_an_ace_bot_laps_faster_than_a_pro_bot_which_laps_faster_than_a_basic_bot() {
    let basic = bot_best_lap_at(BotLevel::Basic);
    let pro = bot_best_lap_at(BotLevel::Pro);
    let ace = bot_best_lap_at(BotLevel::Ace);

    assert!(
        ace < pro && pro < basic,
        "best laps: basic {basic}, pro {pro}, ace {ace}"
    );
}
