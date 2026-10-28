//! The runaway wheel at the sim's public seam: `wheel` on the Brands Patch
//! crossing at moments of the race clock, one kart parked on or beside the
//! wheel's path and stepped through the passes, a prediction run beside
//! the server's race, and eight bots racing past the crossing.

mod common;

use common::{brands_patch, started, ticks, track, with_slot_at, BRANDS_PATCH, DT, SEED};

use sim::tuning::{
    BOT_LINE_RIGHT_OF_CENTRE_M, COUNTDOWN_S, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M, LAPS_PER_RACE,
    TICKS_PER_SECOND, WHEEL_BOUNCES, WHEEL_FIRST_PASS_S, WHEEL_LANDING_SPREAD_M, WHEEL_PERIOD_S,
    WHEEL_RADIUS_M, WHEEL_WARNING_S,
};
use std::sync::OnceLock;

use sim::{step, wheel, KartInput, Phase, Prediction, RaceEvent, State, Wheel, WheelCrossing};

/// Brands Patch's crossing, loaded once: the helpers below ask for it at
/// every sample of the wheel.
fn crossing() -> WheelCrossing {
    static CROSSING: OnceLock<WheelCrossing> = OnceLock::new();
    *CROSSING.get_or_init(|| brands_patch().wheel_crossings()[0])
}

/// The point `right` metres right of the crossing's centre and `ahead`
/// metres along the track from it.
fn on_crossing(ahead: f32, right: f32) -> [f32; 3] {
    let crossing = crossing();
    // Yaw zero faces -Z; forward is (-sin, -cos) and right (cos, -sin) in x, z.
    let (sin, cos) = crossing.yaw.sin_cos();
    let [x, y, z] = crossing.position;
    [
        x - ahead * sin + right * cos,
        y,
        z - ahead * cos - right * sin,
    ]
}

/// `position` as metres right of the crossing's centre and ahead of it.
fn local(position: [f32; 3]) -> [f32; 2] {
    let crossing = crossing();
    let (sin, cos) = crossing.yaw.sin_cos();
    let (dx, dz) = (
        position[0] - crossing.position[0],
        position[2] - crossing.position[2],
    );
    [dx * cos - dz * sin, -dx * sin - dz * cos]
}

/// A race of one kart on Brands Patch, parked from the grid at `position`
/// facing along the track, past its countdown.
fn parked_at(position: [f32; 3]) -> State {
    let sidecar = with_slot_at(BRANDS_PATCH, 0, position, crossing().yaw);
    started(State::new(SEED, 1, track(&sidecar)))
}

/// The race clock at each wheel spin-out `state` reports over `seconds` of
/// standing still.
fn wheel_hits_over(mut state: State, seconds: f32) -> Vec<f32> {
    let mut hits = Vec::new();
    for _ in 0..ticks(seconds) {
        state = step(state, &[KartInput::default()], DT);
        for event in state.events() {
            if let RaceEvent::SpunOut { by: None, .. } = event {
                hits.push(racing_s(&state));
            }
        }
    }
    hits
}

fn racing_s(state: &State) -> f32 {
    let Phase::Racing { elapsed_s, .. } = state.phase() else {
        panic!("not racing: {:?}", state.phase());
    };
    elapsed_s
}

fn pass_start_s(pass: u32) -> f32 {
    WHEEL_FIRST_PASS_S + (pass - 1) as f32 * WHEEL_PERIOD_S
}

/// Three passes and a little over.
fn three_passes_s() -> f32 {
    pass_start_s(4) - WHEEL_WARNING_S
}

/// One straight leg of a pass, wall to wall: where it starts and ends, in
/// metres right of the crossing's centre and ahead of it, and when, on the
/// race clock.
#[derive(Debug, Clone, Copy)]
struct Leg {
    from: [f32; 2],
    to: [f32; 2],
    starts_s: f32,
    ends_s: f32,
}

impl Leg {
    fn at(&self, fraction: f32) -> [f32; 2] {
        [
            self.from[0] + (self.to[0] - self.from[0]) * fraction,
            self.from[1] + (self.to[1] - self.from[1]) * fraction,
        ]
    }
}

/// Pass `pass`'s legs in a race from `seed`, read off `wheel` sampled
/// through the pass: each leg's ends are its first and last samples
/// carried out along it to fractions 0 and 1.
fn legs_of(seed: u64, pass: u32) -> Vec<Leg> {
    static TRACK: OnceLock<sim::Track> = OnceLock::new();
    let track = TRACK.get_or_init(brands_patch);
    let mut groups: Vec<Vec<(f32, [f32; 2], f32)>> = Vec::new();
    let mut last_fraction = f32::INFINITY;
    let start = pass_start_s(pass);
    for step in 1.. {
        let elapsed_s = start + step as f32 * 0.01;
        let Wheel::Rolling {
            pass: rolling,
            position,
            leg_fraction,
            ..
        } = wheel(track, 0, seed, elapsed_s)
        else {
            break;
        };
        assert_eq!(rolling, pass);
        if leg_fraction < last_fraction {
            groups.push(Vec::new());
        }
        last_fraction = leg_fraction;
        groups
            .last_mut()
            .unwrap()
            .push((elapsed_s, local(position), leg_fraction));
    }
    groups
        .into_iter()
        .map(|samples| {
            let (t0, p0, f0) = samples[0];
            let (t1, p1, f1) = *samples.last().unwrap();
            let span = f1 - f0;
            let out = |fraction: f32| {
                let k = (fraction - f0) / span;
                [p0[0] + (p1[0] - p0[0]) * k, p0[1] + (p1[1] - p0[1]) * k]
            };
            let when = |fraction: f32| t0 + (t1 - t0) * (fraction - f0) / span;
            Leg {
                from: out(0.0),
                to: out(1.0),
                starts_s: when(0.0),
                ends_s: when(1.0),
            }
        })
        .collect()
}

/// A spot on pass 1's middle leg, rebound to rebound, as far off the
/// crossing's line as a quarter of the way from either end puts it.
fn on_the_middle_leg() -> [f32; 2] {
    let leg = legs_of(SEED, 1)[1];
    let spot = [leg.at(0.25), leg.at(0.75)]
        .into_iter()
        .max_by(|a, b| a[1].abs().total_cmp(&b[1].abs()))
        .unwrap();
    assert!(
        spot[1].abs() > KART_HALF_LENGTH_M + WHEEL_RADIUS_M,
        "the seed's middle leg runs too close to the crossing's line: {spot:?}"
    );
    spot
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 0.02 && (a[1] - b[1]).abs() < 0.02
}

#[test]
fn the_wheel_is_quiet_until_the_flag_goes_up_before_each_pass_and_rolls_through_it() {
    let track = brands_patch();
    let at = |elapsed_s: f32| wheel(&track, 0, SEED, elapsed_s);
    for pass in 1..=3 {
        let start = pass_start_s(pass);
        let ends = legs_of(SEED, pass).last().unwrap().ends_s;
        assert_eq!(
            at(start - WHEEL_WARNING_S - 0.05),
            Wheel::Quiet,
            "pass {pass}, before the flag"
        );
        for elapsed_s in [start - WHEEL_WARNING_S, start - 0.05] {
            assert!(
                matches!(at(elapsed_s), Wheel::Warning { pass: coming, .. } if coming == pass),
                "pass {pass} flagged at {elapsed_s}"
            );
        }
        for (elapsed_s, through) in [(start + 0.05, 0.0), (ends - 0.05, 1.0)] {
            let Wheel::Rolling {
                pass: rolling,
                fraction,
                ..
            } = at(elapsed_s)
            else {
                panic!("pass {pass} rolling at {elapsed_s}");
            };
            assert_eq!(rolling, pass);
            assert!((fraction - through).abs() < 0.02, "{fraction} through");
        }
        assert_eq!(at(ends + 0.05), Wheel::Quiet, "pass {pass} over");
    }
    assert_eq!(at(0.0), Wheel::Quiet, "at Go");
}

#[test]
fn through_the_warning_the_wheel_waits_at_the_wall_it_rolls_in_from_facing_the_way_it_will_roll() {
    let track = brands_patch();
    let half = crossing().half_width;
    for pass in 1..=4 {
        // Odd passes roll in from the left wall, even ones from the right.
        let side = if pass % 2 == 1 { 1.0 } else { -1.0 };
        let start = pass_start_s(pass);
        let Wheel::Rolling {
            yaw: rolling_yaw, ..
        } = wheel(&track, 0, SEED, start + 0.01)
        else {
            panic!("pass {pass} rolling as it starts");
        };
        for elapsed_s in [start - WHEEL_WARNING_S, start - 0.5, start - 0.01] {
            let Wheel::Warning {
                pass: coming,
                from,
                yaw,
            } = wheel(&track, 0, SEED, elapsed_s)
            else {
                panic!("pass {pass} flagged at {elapsed_s}");
            };
            assert_eq!(coming, pass);
            assert!(
                near(local(from), [-side * half, 0.0]),
                "pass {pass} waits at {from:?}"
            );
            assert_eq!(from[1], crossing().position[1], "on the crossing's surface");
            assert!(
                (yaw - rolling_yaw).abs() < 1e-4,
                "pass {pass}: {yaw} vs {rolling_yaw}"
            );
        }
    }
}

#[test]
fn each_pass_rolls_in_on_the_crossings_line_bounces_twice_and_leaves_through_the_far_wall() {
    let track = brands_patch();
    let half = crossing().half_width;
    let rebound = half - WHEEL_RADIUS_M;
    for pass in 1..=4 {
        // Odd passes roll in from the left wall, even ones from the right.
        let side = if pass % 2 == 1 { 1.0 } else { -1.0 };
        let legs = legs_of(SEED, pass);
        assert_eq!(legs.len(), WHEEL_BOUNCES as usize + 1, "pass {pass}");
        assert!(
            near(legs[0].from, [-side * half, 0.0]),
            "pass {pass} in at {:?}",
            legs[0].from
        );
        let walls = [side * rebound, -side * rebound, side * half];
        for (leg, wall) in legs.iter().zip(walls) {
            assert!((leg.to[0] - wall).abs() < 0.02, "pass {pass}: {leg:?}");
        }
        for pair in legs.windows(2) {
            assert!(near(pair[0].to, pair[1].from), "pass {pass}: {pair:?}");
            assert!((pair[0].ends_s - pair[1].starts_s).abs() < 0.01);
        }
        // The wheel faces the way each leg rolls, turning at each wall.
        for leg in &legs {
            let Wheel::Rolling { yaw, .. } =
                wheel(&track, 0, SEED, (leg.starts_s + leg.ends_s) / 2.0)
            else {
                panic!("pass {pass} rolling mid-leg");
            };
            let (sin, cos) = (yaw - crossing().yaw).sin_cos();
            let ahead_of_yaw = [-sin, cos];
            let length = (leg.to[0] - leg.from[0]).hypot(leg.to[1] - leg.from[1]);
            let rolls = [
                (leg.to[0] - leg.from[0]) / length,
                (leg.to[1] - leg.from[1]) / length,
            ];
            assert!(near(ahead_of_yaw, rolls), "{ahead_of_yaw:?} vs {rolls:?}");
        }
    }
}

#[test]
fn every_landing_lies_within_the_spread_of_the_crossings_line() {
    let landings: Vec<f32> = [SEED, 1, 2, 3]
        .into_iter()
        .flat_map(|seed| (1..=10).flat_map(move |pass| legs_of(seed, pass)))
        .map(|leg| leg.to[1])
        .collect();
    for &ahead in &landings {
        assert!(ahead.abs() <= WHEEL_LANDING_SPREAD_M + 0.02, "{ahead} m");
    }
    // Spread over it, not bunched on the line.
    let (low, high) = landings.iter().fold((0.0f32, 0.0f32), |(low, high), &a| {
        (low.min(a), high.max(a))
    });
    assert!(low < -WHEEL_LANDING_SPREAD_M / 2.0 && WHEEL_LANDING_SPREAD_M / 2.0 < high);
}

#[test]
fn the_same_seed_rolls_the_same_path_and_another_seed_another() {
    let track = brands_patch();
    let positions = |seed: u64| -> Vec<Wheel> {
        (0..3000)
            .map(|step| wheel(&track, 0, seed, pass_start_s(1) + step as f32 * 0.01))
            .collect()
    };
    assert_eq!(positions(SEED), positions(SEED));
    let differ = positions(SEED)
        .into_iter()
        .zip(positions(SEED + 1))
        .any(|pair| match pair {
            (Wheel::Rolling { position: a, .. }, Wheel::Rolling { position: b, .. }) => {
                (a[0] - b[0]).hypot(a[2] - b[2]) > 1.0
            }
            _ => false,
        });
    assert!(differ, "another seed rolled the same path");
}

#[test]
fn a_kart_parked_on_a_bounce_leg_off_the_crossings_line_spins_out_as_the_wheel_reaches_it_by_no_kart(
) {
    let [right, ahead] = on_the_middle_leg();
    let mut state = parked_at(on_crossing(ahead, right));
    let leg = legs_of(SEED, 1)[1];
    loop {
        let previous = state.wheels()[0];
        state = step(state, &[KartInput::default()], DT);
        let elapsed_s = racing_s(&state);
        assert!(elapsed_s < leg.ends_s, "the middle leg missed the kart");
        let Some(event) = state.events().first() else {
            continue;
        };
        assert_eq!(*event, RaceEvent::SpunOut { kart: 0, by: None });
        assert!(
            leg.starts_s <= elapsed_s,
            "hit on the first leg at {elapsed_s}"
        );
        // Hit the tick the wheel came within reach of the kart's box.
        let gap = |wheel: Wheel| match wheel {
            Wheel::Rolling { position, .. } => {
                let [r, a] = local(position);
                (r - right).hypot(a - ahead)
            }
            other => panic!("{other:?}"),
        };
        let corner = KART_HALF_LENGTH_M.hypot(KART_HALF_WIDTH_M);
        assert!(gap(state.wheels()[0]) <= corner + WHEEL_RADIUS_M);
        assert!(gap(previous) > KART_HALF_WIDTH_M + WHEEL_RADIUS_M);
        break;
    }
    assert!(state.kart_readouts()[0].spin_out_s.is_some());
}

/// The least distance, seen from above, between the box of a kart centred
/// at `centre` facing along the track and `legs`, in metres.
fn box_to_path_m(centre: [f32; 2], legs: &[Leg]) -> f32 {
    let mut least = f32::INFINITY;
    for leg in legs {
        for step in 0..=1000 {
            let [right, ahead] = leg.at(step as f32 / 1000.0);
            let across = ((right - centre[0]).abs() - KART_HALF_WIDTH_M).max(0.0);
            let along = ((ahead - centre[1]).abs() - KART_HALF_LENGTH_M).max(0.0);
            least = least.min(across.hypot(along));
        }
    }
    least
}

/// The wheel's spin-outs through pass 1 of a kart parked in the middle of
/// the track, ahead of the whole pass, with its box `gap` metres from the
/// nearest point of the path.
fn hits_parked_ahead_of_the_path(gap: f32) -> Vec<f32> {
    let legs = legs_of(SEED, 1);
    // Ahead of the path the gap only grows the further on, so halving finds it.
    let (mut near, mut far) = (
        legs.iter()
            .map(|leg| leg.from[1].max(leg.to[1]))
            .fold(0.0, f32::max),
        WHEEL_LANDING_SPREAD_M + KART_HALF_LENGTH_M + 2.0 * gap,
    );
    for _ in 0..40 {
        let ahead = (near + far) / 2.0;
        if box_to_path_m([0.0, ahead], &legs) < gap {
            near = ahead;
        } else {
            far = ahead;
        }
    }
    let state = parked_at(on_crossing(far, 0.0));
    let parked = local(state.kart_readouts()[0].position);
    assert!(
        (box_to_path_m(parked, &legs) - gap).abs() < 0.01,
        "{parked:?}"
    );
    wheel_hits_over(state, pass_start_s(2) - WHEEL_WARNING_S)
}

#[test]
fn a_kart_parked_with_its_box_1_1_m_from_the_wheels_path_spins_out_as_the_wheel_passes() {
    let hits = hits_parked_ahead_of_the_path(1.1);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(pass_start_s(1) <= hits[0], "{hits:?}");
}

#[test]
fn a_kart_parked_with_its_box_1_3_m_from_the_wheels_path_is_never_hit() {
    assert_eq!(hits_parked_ahead_of_the_path(1.3), Vec::<f32>::new());
}

#[test]
fn a_kart_parked_beyond_the_wheels_reach_is_never_hit_through_three_passes() {
    let clear = WHEEL_LANDING_SPREAD_M + WHEEL_RADIUS_M + KART_HALF_LENGTH_M + 0.2;
    for ahead in [-clear, clear] {
        assert_eq!(
            wheel_hits_over(parked_at(on_crossing(ahead, 0.0)), three_passes_s()),
            Vec::<f32>::new(),
            "{ahead} m along the track"
        );
    }
}

#[test]
fn a_kart_on_the_crossings_line_by_the_left_wall_is_hit_as_each_odd_pass_rolls_in_and_once_a_pass()
{
    let half = crossing().half_width;
    let state = parked_at(on_crossing(0.0, -(half - 1.5)));
    let hits = wheel_hits_over(state, three_passes_s());
    for pass in [1, 3] {
        let start = pass_start_s(pass);
        assert!(
            hits.iter().any(|&hit| (start..start + 0.2).contains(&hit)),
            "pass {pass} rolling in, hits at {hits:?}"
        );
    }
    for pass in 1..=3 {
        let in_pass = hits
            .iter()
            .filter(|&&hit| (pass_start_s(pass)..pass_start_s(pass + 1)).contains(&hit))
            .count();
        assert!(in_pass <= 1, "pass {pass}, hits at {hits:?}");
    }
}

#[test]
fn the_wheel_draws_nothing_from_the_races_generator_so_item_rolls_are_unchanged() {
    // A race with the wheel and one without, a kart standing beyond its
    // reach: the generator that rolls items is where it was, so the races
    // agree.
    let clear = WHEEL_LANDING_SPREAD_M + WHEEL_RADIUS_M + KART_HALF_LENGTH_M + 0.2;
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(-clear, 0.0), crossing().yaw);
    let mut without: serde_json::Value = serde_json::from_str(&sidecar).unwrap();
    without["wheel_crossings"] = serde_json::json!([]);
    let mut with_wheel = started(State::new(SEED, 1, track(&sidecar)));
    let mut no_wheel = started(State::new(SEED, 1, track(&without.to_string())));
    let mut rolled = false;
    for _ in 0..ticks(three_passes_s()) {
        with_wheel = step(with_wheel, &[KartInput::default()], DT);
        no_wheel = step(no_wheel, &[KartInput::default()], DT);
        rolled |= matches!(with_wheel.wheels()[..], [Wheel::Rolling { .. }]);
        assert_eq!(
            with_wheel.hash(),
            no_wheel.hash(),
            "at {:?}",
            with_wheel.phase()
        );
    }
    assert!(rolled, "the wheel rolled");
}

#[test]
fn through_the_countdown_the_wheel_is_quiet_and_hits_nothing() {
    let [right, ahead] = on_the_middle_leg();
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(ahead, right), crossing().yaw);
    let mut state = State::new(SEED, 1, track(&sidecar));
    while let Phase::Countdown { .. } = state.phase() {
        assert_eq!(state.wheels(), [Wheel::Quiet]);
        state = step(state, &[KartInput::default()], DT);
        assert_eq!(state.events(), []);
    }
}

#[test]
fn through_the_outro_the_wheel_is_quiet_and_hits_nothing() {
    // The race ends as the first pass rolls toward the kart: its human
    // handed to a bot, no kart holds the race open.
    let [right, ahead] = on_the_middle_leg();
    let mut state = parked_at(on_crossing(ahead, right));
    let ends_s = pass_start_s(1) + 0.5;
    while racing_s(&state) < ends_s {
        state = step(state, &[KartInput::default()], DT);
    }
    assert!(matches!(state.wheels()[..], [Wheel::Rolling { .. }]));
    state.hand_to_bot(0);
    state = step(state, &[KartInput::default()], DT);
    assert!(matches!(state.phase(), Phase::Outro { .. }));
    while let Phase::Outro { .. } = state.phase() {
        assert_eq!(state.wheels(), [Wheel::Quiet]);
        state = step(state, &[KartInput::default()], DT);
        assert!(
            !state
                .events()
                .iter()
                .any(|event| matches!(event, RaceEvent::SpunOut { .. })),
            "{:?}",
            state.events()
        );
    }
}

#[test]
fn a_prediction_beside_the_servers_race_reports_the_wheels_spin_out_on_the_same_tick() {
    let [right, ahead] = on_the_middle_leg();
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(ahead, right), crossing().yaw);
    let mut server = State::new(SEED, 1, track(&sidecar));
    let mut prediction = Prediction::new(SEED, 0, track(&sidecar));
    // A client a few ticks ahead of the server, whose snapshots reach it a
    // few ticks late: the prediction steps each server tick's input before
    // the server does, and reconciles with each snapshot when it lands.
    const LEAD_TICKS: u32 = 3;
    let mut snapshots = std::collections::VecDeque::new();
    let (mut server_hit, mut predicted_hit) = (None, None);
    let seconds = (COUNTDOWN_S + legs_of(SEED, 1)[1].ends_s) * TICKS_PER_SECOND as f32;
    for tick in 1..=seconds as u32 {
        prediction.tick(tick, KartInput::default(), &[]);
        if predicted_hit.is_none() && prediction.kart_readout().spin_out_s.is_some() {
            predicted_hit = Some(prediction.phase());
        }
        if tick > LEAD_TICKS {
            server = step(server, &[KartInput::default()], DT);
            let server_tick = tick - LEAD_TICKS;
            snapshots.push_back((server_tick, server.kart_readouts()[0]));
            if server
                .events()
                .contains(&RaceEvent::SpunOut { kart: 0, by: None })
            {
                server_hit.get_or_insert(server.phase());
            }
            if snapshots.len() > 2 * LEAD_TICKS as usize {
                let (at, kart) = snapshots.pop_front().unwrap();
                prediction.reconcile(at, at, &kart);
            }
        }
    }
    let server_hit = server_hit.expect("the server's wheel hit the kart");
    assert_eq!(predicted_hit, Some(server_hit), "the same race clock");
    assert_eq!(prediction.corrections(), 0, "the snapshots agreed");
}

#[test]
fn eight_bots_racing_three_laps_of_brands_patch_are_never_hit_by_the_wheel() {
    // Seeds 1 and 11 each caught out an earlier bot: one sped into a pass
    // it judged clear by a hundredth of a second, the other crawled up to
    // the crossing lifting for passes far ahead. On 80 a bot sped in on a
    // speed between holding its own and flat out.
    for seed in [SEED, 1, 11, 80] {
        bots_race_past_the_wheel(seed);
    }
}

#[test]
fn bots_backing_out_of_a_stall_by_the_crossing_are_never_hit_by_the_wheel() {
    // On 60 a bot turned back toward the crossing from past it; on 87 and
    // 112 one stalled on it backed up into a pass, or set off again from
    // within its reach. On 5 one stopped short of the reach backed up for
    // pass after pass and missed the cap.
    for seed in [5, 60, 87, 112] {
        bots_race_past_the_wheel(seed);
    }
}

#[test]
fn a_bot_standing_short_of_the_crossing_at_go_sets_off_and_clears_it_before_the_first_pass() {
    // Standing where it is, the bot would be in the way of every pass, but
    // it sets off, and even slowly it is long past before the flag. Seed 0
    // rolls the bot no start boost, so it does stand at Go.
    let start = on_crossing(-30.0, BOT_LINE_RIGHT_OF_CENTRE_M);
    let sidecar = with_slot_at(BRANDS_PATCH, 0, start, crossing().yaw);
    let mut state = started(State::with_bots(0, 0, 1, track(&sidecar)));
    state = step(state, &[], DT);
    assert!(
        state.kart_readouts()[0].speed.abs() < 1.0,
        "standing at Go: {:?}",
        state.kart_readouts()[0]
    );
    let clear = WHEEL_LANDING_SPREAD_M + WHEEL_RADIUS_M + KART_HALF_LENGTH_M;
    while racing_s(&state) < pass_start_s(1) - WHEEL_WARNING_S {
        state = step(state, &[], DT);
        if local(state.kart_readouts()[0].position)[1] > clear {
            return;
        }
    }
    panic!(
        "short of the crossing as the flag goes up: {:?}",
        state.kart_readouts()[0]
    );
}

/// Eight bots on Brands Patch from `seed`, raced three laps: none is hit
/// by the wheel, each finishes, and some bot came up to a rolling wheel.
fn bots_race_past_the_wheel(seed: u64) {
    let track = brands_patch();
    assert_eq!(track.wheel_crossings().len(), 1, "the approach's crossing");
    let mut state = started(State::with_bots(seed, 0, 8, track));
    // Ticks a bot spent short of the wheel's reach, within a crossing's
    // width, while the wheel rolled, so the bots were put to the test.
    let reach = WHEEL_LANDING_SPREAD_M + WHEEL_RADIUS_M + KART_HALF_LENGTH_M;
    let mut met = 0;
    for _ in 0..ticks(LAPS_PER_RACE as f32 * 60.0) {
        state = step(state, &[], DT);
        for event in state.events() {
            assert!(
                !matches!(event, RaceEvent::SpunOut { by: None, .. }),
                "seed {seed}: {event:?} at {:?}",
                state.phase()
            );
        }
        if let [Wheel::Rolling { .. }] = state.wheels()[..] {
            met += state
                .kart_readouts()
                .iter()
                .filter(|kart| {
                    let short_of = -local(kart.position)[1];
                    (reach..reach + 2.0 * crossing().half_width).contains(&short_of)
                })
                .count();
        }
        if state
            .standings()
            .iter()
            .all(|s| s.laps_completed >= LAPS_PER_RACE)
        {
            break;
        }
    }
    assert!(
        state
            .standings()
            .iter()
            .all(|s| s.laps_completed >= LAPS_PER_RACE),
        "seed {seed}: every bot raced its three laps"
    );
    assert!(met > 0, "seed {seed}: no bot came up to a rolling wheel");
}

#[test]
fn a_predicted_hit_the_snapshots_overrule_leaves_the_pass_free_to_hit_where_the_server_has_the_kart(
) {
    // The prediction parks the kart on the middle leg and is hit there;
    // the server has it a little further along the wheel's way, so its hit
    // comes a couple of ticks later. A snapshot from between the two puts
    // the kart there, not spun out, and the replay from it must meet the
    // server's hit.
    let [right, ahead] = on_the_middle_leg();
    let leg = legs_of(SEED, 1)[1];
    let length = (leg.to[0] - leg.from[0]).hypot(leg.to[1] - leg.from[1]);
    let further = 0.4 / length;
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(ahead, right), crossing().yaw);
    let mut prediction = Prediction::new(SEED, 0, track(&sidecar));
    let mut tick = 0;
    let idle = KartInput::default();
    while prediction.kart_readout().spin_out_s.is_none() {
        tick += 1;
        prediction.tick(tick, idle, &[]);
        assert!(tick < 60 * 60, "the prediction's wheel never hit the kart");
    }
    let snapshot = sim::KartReadout {
        position: on_crossing(
            ahead + (leg.to[1] - leg.from[1]) * further,
            right + (leg.to[0] - leg.from[0]) * further,
        ),
        spin_out_s: None,
        ..prediction.kart_readout()
    };
    prediction.reconcile(tick, tick, &snapshot);
    assert_eq!(
        prediction.kart_readout().spin_out_s,
        None,
        "the hit is overruled"
    );
    let mut hit = false;
    for _ in 0..6 {
        tick += 1;
        prediction.tick(tick, idle, &[]);
        hit |= prediction.kart_readout().spin_out_s.is_some();
    }
    assert!(hit, "the wheel reached the kart where the server has it");
}

#[test]
fn a_prediction_told_the_race_has_ended_quiets_its_wheel_and_is_never_hit() {
    // The kart is parked on the middle leg; the server's race ends as the
    // first pass rolls toward it, and the client is told so.
    let [right, ahead] = on_the_middle_leg();
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(ahead, right), crossing().yaw);
    let mut prediction = Prediction::new(SEED, 0, track(&sidecar));
    let idle = KartInput::default();
    let mut tick = 0;
    let ends_s = pass_start_s(1) + 0.5;
    while !matches!(prediction.phase(), Phase::Racing { elapsed_s, .. } if elapsed_s >= ends_s) {
        tick += 1;
        prediction.tick(tick, idle, &[]);
    }
    assert!(matches!(prediction.wheels()[..], [Wheel::Rolling { .. }]));
    prediction.end_race();
    for _ in 0..ticks(WHEEL_PERIOD_S) {
        tick += 1;
        prediction.tick(tick, idle, &[]);
        assert_eq!(prediction.wheels(), [Wheel::Quiet]);
        assert_eq!(prediction.kart_readout().spin_out_s, None);
    }
}
