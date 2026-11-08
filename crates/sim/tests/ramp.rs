//! The half-width ramp at Bell Deer's Spoon exit through `step` (ADR 0101):
//! the left half of the road flies off a boost pad's run-up, the right half
//! stays on the ground, and a bot on either half finishes its lap. A hop on
//! the lip earns a trick that boosts the landing (ADR 0102).

mod common;

use common::{
    follow_line, only_kart, started, steer_toward, ticks, track, with_slot_at,
    without_wheel_crossings, BELL_DEER, DT, SEED,
};

use sim::tuning::{
    TOP_SPEED_M_PER_S, TRICK_BOOST_FACTOR, TRICK_BOOST_S, TRICK_EARLY_S, TRICK_LATE_S,
};
use sim::{step, BoostTier, BotLevel, KartInput, KartReadout, State};

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

/// The tuned window in ticks, either side of the kart reaching the lip.
fn early_ticks() -> i32 {
    ticks(TRICK_EARLY_S) as i32
}
fn late_ticks() -> i32 {
    ticks(TRICK_LATE_S) as i32
}
/// A press further from the lip than the window, seen at the pad's boosted
/// speed. Half as long again, since a slope's run reads short in ticks.
fn too_early_tick() -> i32 {
    -early_ticks() * 3 / 2
}

/// A run up the ramp's side and four seconds on, one readout a tick, with the
/// hop held as `hop` says of each tick counted from the first past the lip.
fn flight(right_of_centre: f32, hop: impl Fn(i32) -> bool) -> Vec<KartReadout> {
    let lip_tick = ticks_to_the_lip(right_of_centre);
    let mut state = on_the_run_up(right_of_centre);
    let mut readouts = Vec::new();
    for tick in 0..lip_tick + ticks(4.0) as i32 {
        let input = KartInput {
            hop: hop(tick - lip_tick),
            ..follow_line(&state, 0, right_of_centre)
        };
        state = step(state, &[input], DT);
        readouts.push(only_kart(&state));
    }
    readouts
}

/// How many ticks from the run-up until the kart first stands past the lip.
fn ticks_to_the_lip(right_of_centre: f32) -> i32 {
    let mut state = on_the_run_up(right_of_centre);
    let mut tick = 0;
    while state.standings()[0].spline_progress < LIP_M {
        let input = follow_line(&state, 0, right_of_centre);
        state = step(state, &[input], DT);
        tick += 1;
    }
    tick
}

/// The readout on the first tick the kart stands on the road again past the lip.
fn landing(readouts: &[KartReadout]) -> KartReadout {
    let took_off = readouts
        .iter()
        .rposition(|kart| kart.airborne)
        .expect("it leaves the ground");
    readouts[took_off + 1]
}

fn pressed_at(tick: i32) -> impl Fn(i32) -> bool {
    move |from_the_lip| from_the_lip == tick
}

#[test]
fn a_hop_on_the_lip_earns_a_trick_that_boosts_the_landing() {
    for press in [-early_ticks(), -2, 0, 2, late_ticks() - 1] {
        let readouts = flight(RAMP_SIDE_RIGHT_OF_CENTRE_M, pressed_at(press));
        assert!(
            readouts.iter().any(|kart| kart.trick),
            "a press {press} ticks from the lip earns it"
        );
        let landed = landing(&readouts);
        assert!(!landed.trick, "the landing spends it: press {press}");
        let boost = landed.boost.expect("it lands boosted");
        assert_eq!(boost.tier, BoostTier::Orange, "press {press}");
        assert!(
            (boost.left_s - TRICK_BOOST_S).abs() <= 2.0 * DT,
            "the trick's whole boost from the first grounded tick: {boost:?}"
        );
        assert_eq!(landed.speed, TOP_SPEED_M_PER_S * TRICK_BOOST_FACTOR);
    }
}

#[test]
fn a_hop_too_early_or_too_late_or_none_earns_nothing() {
    for press in [too_early_tick(), late_ticks(), i32::MAX] {
        let readouts = flight(RAMP_SIDE_RIGHT_OF_CENTRE_M, pressed_at(press));
        assert!(
            readouts.iter().all(|kart| !kart.trick),
            "a press {press} ticks from the lip earns nothing"
        );
        // What it lands on is the pad's boost run down, red, or nothing.
        let landed = landing(&readouts);
        assert_ne!(
            landed.boost.map(|boost| boost.tier),
            Some(BoostTier::Orange),
            "press {press}"
        );
    }
}

/// A hop is one try: mashing the button in its flight never earns the trick
/// the first press missed, in the hop's airtime or in the fall that outlasts it.
#[test]
fn a_second_press_in_a_hops_flight_earns_nothing() {
    for (hop, again) in [(too_early_tick(), 0), (-14, 4), (-18, 0)] {
        let readouts = flight(RAMP_SIDE_RIGHT_OF_CENTRE_M, |from_the_lip| {
            from_the_lip == hop || from_the_lip == again
        });
        assert!(
            readouts.iter().all(|kart| !kart.trick),
            "a hop at {hop} and a press at {again}"
        );
    }
}

#[test]
fn a_hop_held_through_the_lip_earns_nothing() {
    let readouts = flight(RAMP_SIDE_RIGHT_OF_CENTRE_M, |from_the_lip| {
        from_the_lip >= too_early_tick()
    });
    assert!(readouts.iter().all(|kart| !kart.trick));
}

#[test]
fn a_hop_on_the_flat_half_beside_the_lip_earns_nothing() {
    let readouts = flight(FLAT_SIDE_RIGHT_OF_CENTRE_M, pressed_at(0));
    assert!(readouts.iter().all(|kart| !kart.trick));
}

/// A drift carried off the lip, let go and pressed again in the fall, earns
/// the trick and keeps its charge: the same charge, a tick past the landing,
/// as the same drift that only held on.
#[test]
fn a_trick_leaves_a_drifts_charge_as_it_was() {
    // Far enough short of the lip for the hop to land and the drift to start
    // on the slope's run-up.
    const DRIFT_HOP_TICKS: i32 = -40;
    let lip_tick = ticks_to_the_lip(RAMP_SIDE_RIGHT_OF_CENTRE_M);
    let drifting = |tricks: bool| {
        let mut state = on_the_run_up(RAMP_SIDE_RIGHT_OF_CENTRE_M);
        let mut readouts: Vec<KartReadout> = Vec::new();
        for tick in 0..lip_tick + ticks(1.5) as i32 {
            let mut input = follow_line(&state, 0, RAMP_SIDE_RIGHT_OF_CENTRE_M);
            if tick - lip_tick >= DRIFT_HOP_TICKS {
                // Let go the tick after the drift takes off, pressed the next.
                let let_go = readouts.last().is_some_and(|kart| {
                    kart.drift.is_some() && kart.airborne && !readouts[readouts.len() - 2].airborne
                });
                input.hop = !(tricks && let_go);
                // A right drift, whose arc keeps the kart on the road ahead.
                input.steer = -0.05;
            }
            state = step(state, &[input], DT);
            readouts.push(only_kart(&state));
        }
        readouts
    };
    let tricked = drifting(true);
    let held = drifting(false);
    assert!(held.iter().all(|kart| !kart.trick));
    assert!(tricked.iter().any(|kart| kart.trick), "it earned the trick");
    let landed = |readouts: &[KartReadout]| {
        let flying = (1..readouts.len())
            .find(|&tick| readouts[tick].drift.is_some() && readouts[tick].airborne)
            .expect("the drift flies");
        (flying..readouts.len())
            .find(|&tick| !readouts[tick].airborne)
            .expect("it lands")
    };
    assert_eq!(landed(&tricked), landed(&held), "the same flight");
    let after = landed(&held) + 1;
    let charge = |readouts: &[KartReadout]| readouts[after].drift.expect("drifting on").charge_s;
    assert_eq!(charge(&tricked), charge(&held));
    assert_eq!(
        tricked[after].boost.map(|boost| boost.tier),
        Some(BoostTier::Orange),
        "and the trick's boost runs alongside it"
    );
}

/// An Ace bot the traffic puts on the ramp's half tricks off the lip; the
/// other levels drive straight off it.
#[test]
fn an_ace_bot_on_the_ramp_tricks_and_the_other_levels_do_not() {
    for (level, tricks) in [
        (BotLevel::Basic, false),
        (BotLevel::Pro, false),
        (BotLevel::Ace, true),
    ] {
        let base = track(BELL_DEER);
        let [dx, dz] = base.direction_at(PAD_BACK_M);
        let [x, y, z] = beside_the_line(PAD_BACK_M, RAMP_SIDE_RIGHT_OF_CENTRE_M - 1.0);
        let sidecar = with_slot_at(
            &without_wheel_crossings(BELL_DEER),
            0,
            [x, y + 0.5, z],
            (-dx).atan2(-dz),
        );
        let mut state = started(State::with_bots(SEED, 0, 1, track(&sidecar), level));
        let mut earned = false;
        let mut flew = false;
        for _ in 0..ticks(5.0) {
            state = step(state, &[], DT);
            let kart = only_kart(&state);
            earned |= kart.trick;
            flew |= kart.airborne;
        }
        assert!(flew, "{level:?} left the ground");
        assert_eq!(earned, tricks, "{level:?}");
    }
}
