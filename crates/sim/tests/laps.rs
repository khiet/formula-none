//! Lap counting and race position at the sim's public seam: drive karts on
//! the oval fixture and read `standings`. The oval's front straight runs
//! along -Z from the start line at z = 0 through its first checkpoints, with
//! the grid a few metres behind the line (layouts/oval.py). Brands Patch
//! joins in for its gated hairpin.

use std::f32::consts::PI;

use sim::tuning::{BOT_LOOKAHEAD_M, LAPS_PER_RACE, TICKS_PER_SECOND};
use sim::{step, KartInput, Phase, Standing, State, Surface, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const BRANDS_PATCH: &str =
    include_str!("../../../assets/tracks/brands-patch/brands-patch.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;

/// Hairpin one on Brands Patch (layouts/brands_patch.py): a 180 degree
/// left-hander of 18 m radius that begins 70 m past the line, with the
/// return straight starting at its exit, 36 m to the left of its entry
/// across the infield grass. The gate at its apex is the third of the lap.
const HAIRPIN_ONE_ENTRY_M: f32 = 70.0;
const HAIRPIN_ONE_EXIT_M: f32 = HAIRPIN_ONE_ENTRY_M + 18.0 * PI;
const APEX_GATE: usize = 2;

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

fn brands_patch() -> Track {
    Track::from_sidecar(BRANDS_PATCH).expect("Brands Patch loads")
}

fn race(kart_count: usize) -> State {
    started(State::new(SEED, kart_count, oval()))
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

fn straight(_: &State, _: usize) -> KartInput {
    throttle()
}

fn only_standing(state: &State) -> Standing {
    let standings = state.standings();
    assert_eq!(standings.len(), 1);
    standings[0]
}

#[test]
fn a_kart_on_the_grid_is_on_lap_one_at_the_start_line_in_first_place() {
    let standing = only_standing(&race(1));

    assert_eq!(standing.position, 1);
    assert_eq!(standing.lap(), 1);
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

fn run(mut state: State, inputs: impl IntoIterator<Item = KartInput>) -> State {
    for input in inputs {
        state = step(state, &[input], DT);
    }
    state
}

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`.
fn yaw(rotation: [f32; 4]) -> f32 {
    let [x, y, z, w] = rotation;
    (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z))
}

/// Full throttle, steering toward the point `target` (x, z) seen from above.
fn steer_toward(state: &State, kart: usize, target: [f32; 2]) -> KartInput {
    const STEER_GAIN: f32 = 4.0;
    let transform = state.kart_transforms()[kart];
    let (dx, dz) = (
        target[0] - transform.position[0],
        target[1] - transform.position[2],
    );
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    // Positive steer turns the kart right; right of a yaw is (cos, -sin) in x, z.
    let (sin, cos) = yaw(transform.rotation).sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    KartInput {
        steer: (rightward * STEER_GAIN).clamp(-1.0, 1.0),
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// Full throttle toward a line `right_of_centre_m` right of the centreline
/// (negative is left), aimed some way ahead of the kart.
fn follow_line(state: &State, kart: usize, right_of_centre_m: f32) -> KartInput {
    let progress = state.standings()[kart].spline_progress;
    let track = state.track();
    let ahead = track.point_at(progress + BOT_LOOKAHEAD_M);
    let further = track.point_at(progress + BOT_LOOKAHEAD_M + 1.0);
    // Right of the direction of travel (dx, dz), seen from above, is (-dz, dx).
    let (tx, tz) = (further[0] - ahead[0], further[2] - ahead[2]);
    let travel = (tx * tx + tz * tz).sqrt().max(f32::EPSILON);
    let target = [
        ahead[0] - tz / travel * right_of_centre_m,
        ahead[2] + tx / travel * right_of_centre_m,
    ];
    steer_toward(state, kart, target)
}

/// A line a little right of the centreline: enough to lap either track on
/// the asphalt and clear of the pipes that stand just left of the
/// centreline in the oval's turns and on the inside of Brands Patch's
/// hairpins.
fn follow_centreline(state: &State, kart: usize) -> KartInput {
    const RIGHT_OF_CENTRE_M: f32 = 2.5;
    follow_line(state, kart, RIGHT_OF_CENTRE_M)
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
            let z = state.kart_transforms()[0].position[2];
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

    let z = state.kart_transforms()[0].position[2];
    assert!(z < -80.0, "the kart is past checkpoints 1 and 2 at z {z}");
    assert_eq!(only_standing(&state).checkpoint, 0);
}

#[test]
fn a_lap_completes_at_the_start_line_after_every_checkpoint_and_records_its_time() {
    let state = drive(race(1), &[Some(follow_centreline)], LAP_S);

    let standing = only_standing(&state);
    assert_eq!(standing.laps_completed, 1);
    assert_eq!(standing.lap(), 2);
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
fn brands_patch_counts_its_checkpoints_the_same_way() {
    let track = brands_patch();
    let lap = track.spline_length();
    let state = started(State::new(SEED, 2, track));
    assert!(
        state.standings()[0].spline_progress > lap - 30.0,
        "the grid is behind the line"
    );

    // Its front straight runs along -Z from the line too; pole drives up it
    // past the first checkpoint while the kart behind stays on the grid.
    let state = drive(state, &[Some(straight), None], 4.0);
    let standings = state.standings();
    assert_eq!(standings[0].checkpoint, 1);
    assert_eq!(standings[0].laps_completed, 0);
    assert_eq!(standings[1].checkpoint, 0);
    assert_eq!(
        standings.iter().map(|s| s.position).collect::<Vec<_>>(),
        [1, 2]
    );
}

/// Up Brands Patch's start straight to the gate before hairpin one, braking
/// over the last stretch to the speed a kart turns off the straight at: at
/// top speed its turning circle is wider than the infield.
fn slowed_past_the_gate_before_hairpin_one() -> State {
    const BRAKE_FROM_M: f32 = 20.0;
    const CUT_SPEED_M_PER_S: f32 = 10.0;
    let mut state = started(State::new(SEED, 1, brands_patch()));
    let gate_z = state.track().checkpoints()[APEX_GATE - 1].position[2];
    for _ in 0..ticks(10.0) {
        let transform = state.kart_transforms()[0];
        let input = if transform.position[2] > gate_z + BRAKE_FROM_M {
            throttle()
        } else {
            KartInput {
                brake: if transform.speed > CUT_SPEED_M_PER_S { 1.0 } else { 0.0 },
                ..KartInput::default()
            }
        };
        state = step(state, &[input], DT);
        if only_standing(&state).checkpoint == APEX_GATE - 1 {
            return state;
        }
    }
    panic!("the start straight ends at gate {}", APEX_GATE - 1);
}

/// Follow the centreline until the kart crosses the start line, or give
/// up after a lap's worth of time. Returns the state and whether it crossed.
fn round_to_the_start_line(mut state: State) -> (State, bool) {
    let lap = state.track().spline_length();
    for _ in 0..ticks(60.0) {
        let before = only_standing(&state).spline_progress;
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
        if only_standing(&state).spline_progress < before - lap / 2.0 {
            return (state, true);
        }
    }
    (state, false)
}

#[test]
fn cutting_hairpin_one_across_the_infield_skips_its_apex_gate_and_the_lap_does_not_count() {
    let mut state = slowed_past_the_gate_before_hairpin_one();

    // Straight across the infield to the top of the return straight.
    let top_of_return = state.track().point_at(HAIRPIN_ONE_EXIT_M + 10.0);
    let mut drove_the_grass = false;
    for _ in 0..ticks(10.0) {
        let input = steer_toward(&state, 0, [top_of_return[0], top_of_return[2]]);
        state = step(state, &[input], DT);
        drove_the_grass |= state.kart_surface(0) == Surface::Grass;
        if state.kart_transforms()[0].position[0] < top_of_return[0] + 4.0 {
            break;
        }
    }
    let x = state.kart_transforms()[0].position[0];
    assert!(
        x < top_of_return[0] + 4.0,
        "the kart reached the return straight: x {x}, return at x {}",
        top_of_return[0]
    );
    assert!(drove_the_grass, "the cut crosses the infield grass");
    assert_eq!(only_standing(&state).checkpoint, APEX_GATE - 1);

    let (state, crossed_the_line) = round_to_the_start_line(state);

    let standing = only_standing(&state);
    assert!(crossed_the_line, "the kart got round to the line");
    assert_eq!(
        standing.checkpoint,
        APEX_GATE - 1,
        "no later gate counts while the apex gate is missed"
    );
    assert_eq!(standing.laps_completed, 0);
    assert_eq!(standing.best_lap_s, None);
}

#[test]
fn rounding_hairpin_one_over_the_inside_sand_credits_the_apex_gate_and_the_lap_counts() {
    // The asphalt line, except through the hairpin, where the kart aims for
    // the sand inside the pipes: the run-off reaches 12 m from the
    // centreline and the pipes stand 4.5 m in.
    const INSIDE_SAND_M: f32 = -9.0;
    let inside_line = |state: &State| {
        let ahead = state.standings()[0].spline_progress + BOT_LOOKAHEAD_M;
        if (HAIRPIN_ONE_ENTRY_M..HAIRPIN_ONE_EXIT_M).contains(&ahead) {
            follow_line(state, 0, INSIDE_SAND_M)
        } else {
            follow_centreline(state, 0)
        }
    };
    let mut state = started(State::new(SEED, 1, brands_patch()));
    let mut surface_at_apex_gate = None;
    for _ in 0..ticks(90.0) {
        let before = only_standing(&state).checkpoint;
        let input = inside_line(&state);
        state = step(state, &[input], DT);
        let standing = only_standing(&state);
        if before != APEX_GATE && standing.checkpoint == APEX_GATE {
            surface_at_apex_gate = Some(state.kart_surface(0));
        }
        if standing.laps_completed == 1 {
            break;
        }
    }

    assert_eq!(surface_at_apex_gate, Some(Surface::Sand));
    let standing = only_standing(&state);
    assert_eq!(standing.laps_completed, 1);
    assert!(standing.best_lap_s.is_some(), "the lap was timed");
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
    let [idle, driven] = state.kart_transforms()[..] else {
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
fn three_laps_of_the_oval_end_with_the_lap_counter_at_three_and_a_best_lap() {
    let mut state = race(1);
    let mut lap_times = Vec::new();
    let mut laps_seen = 0;
    for _ in 0..ticks(3.0 * LAP_S) {
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
        let standing = only_standing(&state);
        if standing.laps_completed > laps_seen {
            laps_seen = standing.laps_completed;
            lap_times.push(standing.best_lap_s.expect("a lap just completed"));
        }
        if laps_seen == LAPS_PER_RACE {
            break;
        }
    }

    let standing = only_standing(&state);
    assert_eq!(standing.laps_completed, 3);
    assert_eq!(standing.lap(), 3, "the counter stays on the last lap");
    assert_eq!(lap_times.len(), 3);
    // The first lap starts from a standstill, so a flying lap is the best.
    let best = standing.best_lap_s.expect("three laps were completed");
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
