//! The runaway wheel at the sim's public seam: `wheel` on the Brands Patch
//! crossing at moments of the race clock, one kart parked on or beside the
//! crossing and stepped through the passes, a prediction run beside the
//! server's race, and eight bots racing past the crossing.

mod common;

use common::{brands_patch, started, ticks, track, with_slot_at, BRANDS_PATCH, DT, SEED};

use sim::tuning::{
    COUNTDOWN_S, KART_HALF_WIDTH_M, LAPS_PER_RACE, TICKS_PER_SECOND, WHEEL_FIRST_PASS_S,
    WHEEL_PASS_S, WHEEL_PERIOD_S, WHEEL_RADIUS_M, WHEEL_WARNING_S,
};
use sim::{step, wheel, KartInput, Phase, Prediction, RaceEvent, State, Wheel, WheelCrossing};

fn crossing() -> WheelCrossing {
    brands_patch().wheel_crossings()[0]
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
                let Phase::Racing { elapsed_s, .. } = state.phase() else {
                    panic!("a hit outside the race: {:?}", state.phase());
                };
                hits.push(elapsed_s);
            }
        }
    }
    hits
}

fn pass_start_s(pass: u32) -> f32 {
    WHEEL_FIRST_PASS_S + (pass - 1) as f32 * WHEEL_PERIOD_S
}

/// Three passes and a little over.
fn three_passes_s() -> f32 {
    pass_start_s(3) + WHEEL_PASS_S + 1.0
}

#[test]
fn the_wheel_is_quiet_until_the_flag_goes_up_before_each_pass_and_rolls_through_it() {
    let track = brands_patch();
    let at = |elapsed_s: f32| wheel(&track, 0, elapsed_s);
    for pass in 1..=3 {
        let start = pass_start_s(pass);
        let before_flag = start - WHEEL_WARNING_S - 0.05;
        assert_eq!(
            at(before_flag),
            Wheel::Quiet,
            "pass {pass}, before the flag"
        );
        assert_eq!(at(start - WHEEL_WARNING_S), Wheel::Warning, "pass {pass}");
        assert_eq!(at(start - 0.05), Wheel::Warning, "pass {pass}");
        // The clock's sum lands a hair either side of a pass's end.
        for fraction in [0.0, 0.5, 0.999] {
            let Wheel::Rolling {
                pass: rolling,
                fraction: through,
                ..
            } = at(start + fraction * WHEEL_PASS_S)
            else {
                panic!("pass {pass} rolling at {fraction}");
            };
            assert_eq!(rolling, pass);
            assert!((through - fraction).abs() < 1e-4, "{through} through");
        }
        assert_eq!(
            at(start + WHEEL_PASS_S + 0.05),
            Wheel::Quiet,
            "pass {pass} over"
        );
    }
    assert_eq!(at(0.0), Wheel::Quiet, "at Go");
}

#[test]
fn each_pass_rolls_wall_to_wall_square_to_the_track_the_other_way_from_the_last() {
    let track = brands_patch();
    let crossing = crossing();
    let right_of_centre = |position: [f32; 3]| {
        let (sin, cos) = crossing.yaw.sin_cos();
        let (dx, dz) = (
            position[0] - crossing.position[0],
            position[2] - crossing.position[2],
        );
        let ahead = -dx * sin - dz * cos;
        assert!(ahead.abs() < 1e-3, "on the crossing's line: {ahead} m off");
        dx * cos - dz * sin
    };
    let ends = |pass: u32| {
        [0.0, 0.999].map(|fraction| {
            match wheel(&track, 0, pass_start_s(pass) + fraction * WHEEL_PASS_S) {
                Wheel::Rolling { position, .. } => right_of_centre(position),
                other => panic!("pass {pass} at {fraction}: {other:?}"),
            }
        })
    };
    let half = crossing.half_width;
    assert_eq!(2.0 * half, 24.0, "wall to wall");
    for pass in [1, 3] {
        let [from, to] = ends(pass);
        assert!(
            (from + half).abs() < 0.05 && (to - half).abs() < 0.05,
            "{from} to {to}"
        );
    }
    let [from, to] = ends(2);
    assert!(
        (from - half).abs() < 0.05 && (to + half).abs() < 0.05,
        "{from} to {to}"
    );
}

#[test]
fn a_kart_parked_on_the_wheels_path_spins_out_as_the_first_pass_reaches_it_by_no_kart() {
    let mut state = parked_at(on_crossing(0.0, 0.0));
    let first = pass_start_s(1);
    // The first pass rolls from the left wall; the kart, facing along the
    // track, is reached once the wheel is within its reach of its left side.
    let reach_s = first
        + (crossing().half_width - KART_HALF_WIDTH_M - WHEEL_RADIUS_M)
            / (2.0 * crossing().half_width)
            * WHEEL_PASS_S;
    let mut hit = None;
    while hit.is_none() {
        state = step(state, &[KartInput::default()], DT);
        let Phase::Racing { elapsed_s, .. } = state.phase() else {
            panic!("the race ended");
        };
        assert!(
            elapsed_s < first + WHEEL_PASS_S,
            "the first pass missed the kart"
        );
        if let Some(event) = state.events().first() {
            assert_eq!(*event, RaceEvent::SpunOut { kart: 0, by: None });
            hit = Some(elapsed_s);
        }
    }
    let hit = hit.unwrap();
    assert!(
        (hit - reach_s).abs() <= DT,
        "hit at {hit} s, the wheel reaches the kart at {reach_s} s"
    );
    assert!(state.kart_readouts()[0].spin_out_s.is_some());
}

#[test]
fn a_kart_parked_beside_the_crossing_is_never_hit_through_three_passes() {
    for ahead in [-3.0, 3.0] {
        let state = parked_at(on_crossing(ahead, 0.0));
        assert_eq!(
            wheel_hits_over(state, three_passes_s()),
            Vec::<f32>::new(),
            "{ahead} m along the track"
        );
    }
}

#[test]
fn a_kart_standing_on_the_path_is_hit_once_a_pass() {
    let hits = wheel_hits_over(parked_at(on_crossing(0.0, 0.0)), three_passes_s());
    assert_eq!(hits.len(), 3, "hit at {hits:?}");
    for (pass, hit) in (1..).zip(hits) {
        let start = pass_start_s(pass);
        assert!(
            (start..=start + WHEEL_PASS_S).contains(&hit),
            "pass {pass} at {hit}"
        );
    }
}

#[test]
fn through_the_countdown_the_wheel_is_quiet_and_hits_nothing() {
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(0.0, 0.0), crossing().yaw);
    let mut state = State::new(SEED, 1, track(&sidecar));
    while let Phase::Countdown { .. } = state.phase() {
        assert_eq!(state.wheels(), [Wheel::Quiet]);
        state = step(state, &[KartInput::default()], DT);
        assert_eq!(state.events(), []);
    }
}

#[test]
fn through_the_outro_the_wheel_is_quiet_and_hits_nothing() {
    // The race ends as the first pass is about to reach the kart: its
    // human handed to a bot, no kart holds the race open.
    let mut state = parked_at(on_crossing(0.0, 0.0));
    let ends_s = pass_start_s(1) + 0.3 * WHEEL_PASS_S;
    loop {
        let Phase::Racing { elapsed_s, .. } = state.phase() else {
            panic!("the race ended early");
        };
        if elapsed_s >= ends_s {
            break;
        }
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
    let position = on_crossing(0.0, 0.0);
    let sidecar = with_slot_at(BRANDS_PATCH, 0, position, crossing().yaw);
    let mut server = State::new(SEED, 1, track(&sidecar));
    let mut prediction = Prediction::new(SEED, 0, track(&sidecar));
    // A client a few ticks ahead of the server, whose snapshots reach it a
    // few ticks late: the prediction steps each server tick's input before
    // the server does, and reconciles with each snapshot when it lands.
    const LEAD_TICKS: u32 = 3;
    let mut snapshots = std::collections::VecDeque::new();
    let (mut server_hit, mut predicted_hit) = (None, None);
    let seconds = (COUNTDOWN_S + pass_start_s(1) + WHEEL_PASS_S) * TICKS_PER_SECOND as f32;
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
    // the crossing lifting for passes far ahead.
    for seed in [SEED, 1, 11] {
        bots_race_past_the_wheel(seed);
    }
}

/// Eight bots on Brands Patch from `seed`, raced three laps: none is hit
/// by the wheel, each finishes, and some bot came up to a rolling wheel.
fn bots_race_past_the_wheel(seed: u64) {
    let track = brands_patch();
    assert_eq!(track.wheel_crossings().len(), 1, "the approach's crossing");
    let crossing = track.wheel_crossings()[0];
    let mut state = started(State::with_bots(seed, 0, 8, track));
    // Ticks a bot spent within a pass's travel short of the crossing while
    // the wheel rolled, so the bots were put to the test.
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
                    let (sin, cos) = crossing.yaw.sin_cos();
                    let (dx, dz) = (
                        kart.position[0] - crossing.position[0],
                        kart.position[2] - crossing.position[2],
                    );
                    let short_of = dx * sin + dz * cos;
                    (0.0..2.0 * crossing.half_width).contains(&short_of)
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
    // The prediction parks the kart on the crossing's centre and is hit
    // there; the server has it a little further along the wheel's way, so
    // its hit comes a couple of ticks later. A snapshot from between the
    // two puts the kart there, not spun out, and the replay from it must
    // meet the server's hit.
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(0.0, 0.0), crossing().yaw);
    let mut prediction = Prediction::new(SEED, 0, track(&sidecar));
    let mut tick = 0;
    let idle = KartInput::default();
    while prediction.kart_readout().spin_out_s.is_none() {
        tick += 1;
        prediction.tick(tick, idle, &[]);
        assert!(tick < 60 * 60, "the prediction's wheel never hit the kart");
    }
    let snapshot = sim::KartReadout {
        position: on_crossing(0.0, 0.4),
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
    // The kart is parked on the crossing; the server's race ends just as
    // the first pass is about to reach it, and the client is told so.
    let sidecar = with_slot_at(BRANDS_PATCH, 0, on_crossing(0.0, 0.0), crossing().yaw);
    let mut prediction = Prediction::new(SEED, 0, track(&sidecar));
    let idle = KartInput::default();
    let mut tick = 0;
    let ends_s = pass_start_s(1) + 0.3 * WHEEL_PASS_S;
    while !matches!(prediction.phase(), Phase::Racing { elapsed_s, .. } if elapsed_s >= ends_s) {
        tick += 1;
        prediction.tick(tick, idle, &[]);
    }
    assert!(matches!(prediction.wheels()[..], [Wheel::Rolling { .. }]));
    prediction.end_race();
    for _ in 0..ticks(WHEEL_PASS_S) {
        tick += 1;
        prediction.tick(tick, idle, &[]);
        assert_eq!(prediction.wheels(), [Wheel::Quiet]);
        assert_eq!(prediction.kart_readout().spin_out_s, None);
    }
}
