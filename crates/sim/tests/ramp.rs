//! The half-width ramp at Bell Deer's Spoon exit through `step` (ADR 0101):
//! the left half of the road flies off a boost pad's run-up, the right half
//! stays on the ground, and a bot on either half finishes its lap.

mod common;

use common::{
    follow_line, only_kart, started, steer_toward, ticks, track, with_slot_at,
    without_wheel_crossings, BELL_DEER, DT, SEED,
};

use sim::tuning::TOP_SPEED_M_PER_S;
use sim::{step, BotLevel, State};

/// `back-straight` starts this far into the lap (layouts/bell_deer.py); the
/// pad's back edge, the ramp's foot and its lip stand 32, 36 and 44 m along it.
const BACK_STRAIGHT_M: f32 = 916.63;
const PAD_BACK_M: f32 = BACK_STRAIGHT_M + 32.0;
const FOOT_M: f32 = BACK_STRAIGHT_M + 36.0;
const LIP_M: f32 = BACK_STRAIGHT_M + 44.0;
/// Far enough short of the pad to reach top speed on the way, and out of
/// Spoon's tyre wall, which stands left of the centreline.
const RUN_UP_M: f32 = BACK_STRAIGHT_M - 8.0;
const RAMP_SIDE_RIGHT_OF_CENTRE_M: f32 = -3.0;
const FLAT_SIDE_RIGHT_OF_CENTRE_M: f32 = 3.0;
/// More than the slope gives a boosted kart and less than a kart caught on an
/// edge of the mesh is thrown at, 7 m/s or more.
const MOST_CLIMB_M_PER_S: f32 = 6.0;

/// The world point `right_of_centre` metres right of the centreline at `metres`.
fn beside_the_line(metres: f32, right_of_centre: f32) -> [f32; 3] {
    let base = track(BELL_DEER);
    let point = base.point_at(metres);
    let [dx, dz] = base.direction_at(metres);
    [
        point[0] - dz * right_of_centre,
        point[1],
        point[2] + dx * right_of_centre,
    ]
}

/// A kart alone on Bell Deer on the run-up, on its line.
fn on_the_run_up(right_of_centre: f32) -> State {
    let base = track(BELL_DEER);
    let [dx, dz] = base.direction_at(RUN_UP_M);
    let [x, y, z] = beside_the_line(RUN_UP_M, right_of_centre);
    let sidecar = with_slot_at(
        &without_wheel_crossings(BELL_DEER),
        0,
        [x, y + 0.5, z],
        (-dx).atan2(-dz),
    );
    started(State::new(SEED, 1, track(&sidecar)))
}

/// What a kart did from the run-up until it landed or ran 80 m past the lip.
#[derive(Debug, Default)]
struct Run {
    boosted: bool,
    took_off_at: Option<f32>,
    /// Spline progress and speed on the tick it came back to the ground.
    landed: Option<(f32, f32)>,
    passed_the_lip: bool,
    fastest_climb: f32,
}

fn up_the_straight(right_of_centre: f32) -> Run {
    let mut state = on_the_run_up(right_of_centre);
    let mut run = Run::default();
    for _ in 0..ticks(10.0) {
        let input = follow_line(&state, 0, right_of_centre);
        state = step(state, &[input], DT);
        let kart = only_kart(&state);
        let progress = state.standings()[0].spline_progress;
        run.passed_the_lip |= progress > LIP_M;
        run.fastest_climb = run.fastest_climb.max(kart.velocity[1]);
        run.boosted |= (PAD_BACK_M..LIP_M).contains(&progress) && kart.boost.is_some();
        // Past the settling on the spawn, which starts it a little above the road.
        if progress < PAD_BACK_M {
            continue;
        }
        match (run.took_off_at, kart.airborne) {
            (None, true) => run.took_off_at = Some(progress),
            (Some(_), false) => {
                run.landed = Some((progress, kart.speed));
                break;
            }
            _ => {}
        }
        if progress > LIP_M + 80.0 {
            break;
        }
    }
    run
}

#[test]
fn a_kart_on_the_ramps_side_flies_and_lands_boosted_before_the_next_checkpoint() {
    let run = up_the_straight(RAMP_SIDE_RIGHT_OF_CENTRE_M);
    assert!(run.boosted, "the pad on the run-up boosts it: {run:?}");
    let took_off = run.took_off_at.expect("it leaves the ground");
    assert!(took_off > LIP_M, "off the lip, not the foot: {run:?}");
    assert!(run.fastest_climb < MOST_CLIMB_M_PER_S, "{run:?}");
    let (landed, speed) = run.landed.expect("it lands");
    let checkpoints = track(BELL_DEER).checkpoint_progress().to_vec();
    let next = checkpoints
        .iter()
        .find(|&&progress| progress > took_off)
        .expect("a checkpoint ahead");
    assert!(landed < *next, "lands short of {next}: {run:?}");
    assert!(speed > TOP_SPEED_M_PER_S, "lands at boosted speed: {run:?}");
}

#[test]
fn a_kart_on_the_flat_side_never_leaves_the_ground_or_meets_the_pad() {
    let run = up_the_straight(FLAT_SIDE_RIGHT_OF_CENTRE_M);
    assert!(run.passed_the_lip, "{run:?}");
    assert_eq!(run.took_off_at, None, "{run:?}");
    assert!(!run.boosted, "{run:?}");
}

#[test]
fn a_kart_straddling_the_ramps_edge_is_not_stopped_by_it() {
    let run = up_the_straight(0.0);
    assert!(run.passed_the_lip, "{run:?}");
    assert!(
        run.fastest_climb < MOST_CLIMB_M_PER_S,
        "not thrown: {run:?}"
    );
}

/// Cutting in from the flat half under the slope's high end, where the slope's
/// underside is open: nothing holds the kart there and nothing throws it.
#[test]
fn a_kart_cutting_in_under_the_high_end_runs_on_unheld() {
    let base = track(BELL_DEER);
    let [dx, dz] = base.direction_at(LIP_M);
    let mut state = on_the_run_up(FLAT_SIDE_RIGHT_OF_CENTRE_M);
    let mut most_climb = 0.0_f32;
    let mut slowest = f32::MAX;
    for _ in 0..ticks(10.0) {
        let progress = state.standings()[0].spline_progress;
        if progress > LIP_M + 10.0 {
            break;
        }
        let right = if progress < LIP_M - 10.0 {
            FLAT_SIDE_RIGHT_OF_CENTRE_M
        } else {
            RAMP_SIDE_RIGHT_OF_CENTRE_M
        };
        let ahead = base.point_at(progress + 8.0);
        let input = steer_toward(&state, 0, [ahead[0] - dz * right, ahead[2] + dx * right]);
        state = step(state, &[input], DT);
        if progress > FOOT_M {
            let kart = only_kart(&state);
            most_climb = most_climb.max(kart.velocity[1]);
            slowest = slowest.min(kart.speed);
        }
    }
    let progress = state.standings()[0].spline_progress;
    assert!(
        progress > LIP_M + 10.0,
        "it runs on past the lip: {progress}"
    );
    assert!(slowest > 0.5 * TOP_SPEED_M_PER_S, "not held: {slowest}");
    assert!(most_climb < MOST_CLIMB_M_PER_S, "not thrown: {most_climb}");
}

/// Bots run the flat half (`BOT_LINE_RIGHT_OF_CENTRE_M`), and traffic puts some
/// on the ramp's: every bot finishes either way.
#[test]
fn eight_bots_finish_whichever_half_of_the_road_the_traffic_gives_them() {
    let mut up_the_slope = 0;
    for level in [BotLevel::Basic, BotLevel::Pro, BotLevel::Ace] {
        let mut state = started(State::with_bots(SEED, 0, 8, track(BELL_DEER), level));
        for _ in 0..ticks(240.0) {
            state = step(state, &[], DT);
            let standings = state.standings();
            if standings.iter().all(|kart| kart.finish_time_s.is_some()) {
                break;
            }
            up_the_slope += state
                .kart_readouts()
                .iter()
                .zip(&standings)
                .filter(|(kart, standing)| {
                    (FOOT_M..LIP_M).contains(&standing.spline_progress) && kart.position[1] > 0.5
                })
                .count();
        }
        let standings = state.standings();
        assert!(
            standings.iter().all(|kart| kart.finish_time_s.is_some()),
            "{level:?}: {standings:?}"
        );
    }
    assert!(up_the_slope > 0, "some bot took the ramp");
}
