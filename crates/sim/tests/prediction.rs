//! Prediction of the own kart at its public seam: `Prediction::tick` with
//! each input the client sends and `Prediction::reconcile` with each
//! snapshot, against a server played here by `step` on a race of its own,
//! applying one sequence per tick as the room's race does (ADR 0019). The
//! network between them is a fixed delay each way in ticks, with the
//! inputs it loses picked by sequence. The scripted driver is the bot's
//! driving, reading the kart as the client draws it, so it hops into the
//! oval's drifts the way a player would.

use std::collections::VecDeque;

use sim::bot::{self, Memory};
use sim::tuning::{
    RECONCILIATION_BLEND_TICKS, RECONCILIATION_TOLERANCE_M, RECONCILIATION_VISIBLE_CORRECTION_M,
    SNAPSHOTS_PER_SECOND, TICKS_PER_SECOND,
};
use sim::{step, KartInput, Phase, Prediction, SnapshotKart, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const SEED: u64 = 0x5EED;
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;
/// 100 ms round trip at the tick rate, split evenly.
const ONE_WAY_TICKS: u32 = 3;

fn oval() -> Track {
    Track::from_sidecar(OVAL).expect("the oval fixture loads")
}

/// What the snapshot of `state` says of its first kart.
fn snapshot_of(state: &State) -> SnapshotKart {
    let transform = state.kart_transforms()[0];
    let standing = state.standings()[0];
    SnapshotKart {
        position: transform.position,
        rotation: transform.rotation,
        velocity: transform.velocity,
        drift: transform.drift,
        held_item: transform.held_item,
        lap: standing.lap(),
        checkpoint: standing.checkpoint,
        spline_progress: standing.spline_progress,
    }
}

/// A snapshot on its way to the client.
struct InFlight {
    arrives: u32,
    tick: u32,
    acknowledged: u32,
    kart: SnapshotKart,
}

/// What a run saw: every reconciliation's correction, in metres.
struct Run {
    prediction: Prediction,
    corrections_m: Vec<f32>,
}

/// One kart raced for `ticks` server ticks: once the first input lands
/// the server applies the next sequence each tick, the last controls
/// standing in for one that never arrived, and snapshots every other tick;
/// the client starts its countdown when the race's first message reaches
/// it and sends one input per tick. Inputs whose sequence `lost` picks
/// never arrive.
fn race(ticks: u32, lost: impl Fn(u32) -> bool) -> Run {
    let mut server = State::new(SEED, 1, oval());
    let mut prediction = Prediction::new(SEED, 0, oval());
    let mut memory = Memory::default();
    let mut inputs_in_flight: VecDeque<(u32, u32, KartInput)> = VecDeque::new();
    let mut snapshots_in_flight: VecDeque<InFlight> = VecDeque::new();
    let mut arrived = Vec::new();
    let mut applied = (0, KartInput::default());
    let mut sequence = 0;
    let mut corrections_m = Vec::new();
    for tick in 1..=ticks {
        while inputs_in_flight
            .front()
            .is_some_and(|&(arrives, _, _)| arrives <= tick)
        {
            let (_, number, input) = inputs_in_flight.pop_front().unwrap();
            arrived.push((number, input));
        }
        if applied.0 > 0 || !arrived.is_empty() {
            let next = applied.0 + 1;
            let input = arrived
                .iter()
                .find(|&&(number, _)| number == next)
                .map_or(applied.1, |&(_, input)| input);
            applied = (next, input);
        }
        server = step(server, &[applied.1], DT);
        if tick.is_multiple_of(TICKS_PER_SNAPSHOT) {
            snapshots_in_flight.push_back(InFlight {
                arrives: tick + ONE_WAY_TICKS,
                tick,
                acknowledged: applied.0,
                kart: snapshot_of(&server),
            });
        }

        if tick <= ONE_WAY_TICKS {
            continue;
        }
        while snapshots_in_flight
            .front()
            .is_some_and(|snapshot| snapshot.arrives <= tick)
        {
            let snapshot = snapshots_in_flight.pop_front().unwrap();
            corrections_m.push(prediction.reconcile(
                snapshot.tick,
                snapshot.acknowledged,
                &snapshot.kart,
            ));
        }
        let input = bot::drive(
            prediction.track(),
            &[prediction.kart_transform()],
            &[prediction.standing()],
            0,
            &mut memory,
            DT,
        );
        sequence += 1;
        prediction.tick(sequence, input, &[]);
        if !lost(sequence) {
            inputs_in_flight.push_back((tick + 1 + ONE_WAY_TICKS, sequence, input));
        }
    }
    Run {
        prediction,
        corrections_m,
    }
}

fn seconds(seconds: f32) -> u32 {
    (seconds * TICKS_PER_SECOND as f32) as u32
}

#[test]
fn at_a_steady_round_trip_the_snapshots_agree_with_the_prediction() {
    let run = race(seconds(20.0), |_| false);
    assert_eq!(run.prediction.corrections(), 0);
    let largest = run.corrections_m.iter().copied().fold(0.0, f32::max);
    assert!(largest <= RECONCILIATION_TOLERANCE_M, "moved {largest} m");
}

#[test]
fn a_lost_input_is_corrected_by_less_than_a_player_can_see_and_counted() {
    // Every fiftieth input lost: two percent, through the drifts.
    let run = race(seconds(20.0), |sequence| sequence % 50 == 0);
    assert!(run.prediction.corrections() > 0, "nothing was corrected");
    let largest = run.corrections_m.iter().copied().fold(0.0, f32::max);
    assert!(
        largest < RECONCILIATION_VISIBLE_CORRECTION_M,
        "moved {largest} m"
    );
    let per_second = run.prediction.corrections_per_second();
    let raced_s = (seconds(20.0) - seconds(sim::tuning::COUNTDOWN_S)) as f32 * DT;
    assert!(
        (per_second - run.prediction.corrections() as f32 / raced_s).abs() < 0.05,
        "{per_second} per second"
    );
}

/// A prediction on the grid, its countdown under way and its inputs
/// acknowledged by a snapshot that agreed.
fn on_the_grid() -> (Prediction, SnapshotKart) {
    let mut prediction = Prediction::new(SEED, 0, oval());
    for sequence in 1..=10 {
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    let grid = snapshot_of(&State::new(SEED, 1, oval()));
    prediction.reconcile(10, 4, &grid);
    assert!(matches!(prediction.phase(), Phase::Countdown { .. }));
    (prediction, grid)
}

#[test]
fn a_correction_is_blended_from_where_the_kart_was_drawn_over_the_tuned_ticks() {
    let (mut prediction, grid) = on_the_grid();
    let drawn = prediction.kart_transform().position;
    let mut aside = grid;
    aside.position[0] += 1.0;

    let moved = prediction.reconcile(12, 5, &aside);
    assert!((moved - 1.0).abs() < 1e-4, "moved {moved} m");
    assert_eq!(prediction.corrections(), 1);
    assert_eq!(prediction.kart_transform().position, drawn, "no jump");
    let mut sequence = 10;
    for tick in 1..=RECONCILIATION_BLEND_TICKS {
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
        let x = prediction.kart_transform().position[0];
        let expected = drawn[0] + tick as f32 / RECONCILIATION_BLEND_TICKS as f32;
        assert!((x - expected).abs() < 1e-4, "tick {tick}: x {x}");
    }
}

#[test]
fn a_snapshot_older_than_one_held_changes_nothing() {
    let (mut prediction, grid) = on_the_grid();
    let mut aside = grid;
    aside.position[0] += 1.0;
    assert_eq!(prediction.reconcile(8, 3, &aside), 0.0);
    assert_eq!(prediction.reconcile(10, 4, &aside), 0.0);
    assert_eq!(prediction.corrections(), 0);
}

#[test]
fn the_predicted_countdown_runs_ahead_of_the_snapshots_by_the_inputs_in_flight() {
    let (prediction, _) = on_the_grid();
    // Input 4 was applied at tick 10, so input 10 meets tick 16.
    let Phase::Countdown { remaining_s } = prediction.phase() else {
        panic!("still counting down");
    };
    let expected = sim::tuning::COUNTDOWN_S - 16.0 * DT;
    assert!(
        (remaining_s - expected).abs() < 1e-4,
        "{remaining_s} s left"
    );
}

#[test]
fn the_snapshots_item_is_taken_without_a_correction_and_firing_it_is_predicted() {
    use sim::tuning::{ITEM_BOOST_FACTOR, TOP_SPEED_M_PER_S};
    use sim::Item;
    let (mut prediction, grid) = on_the_grid();
    assert_eq!(prediction.kart_transform().held_item, None);

    // The server rolled the kart a boost from a box the prediction did not
    // see taken; the kart is where it was predicted, so nothing moves.
    let mut holding = grid;
    holding.held_item = Some(Item::Boost);
    assert_eq!(prediction.reconcile(12, 5, &holding), 0.0);
    assert_eq!(prediction.corrections(), 0);
    assert_eq!(prediction.kart_transform().held_item, Some(Item::Boost));

    // Past Go, the press fires the boost the server gave: the predicted
    // kart is at the boosted speed the tick it is pressed.
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let mut sequence = 10;
    while !matches!(prediction.phase(), Phase::Racing { .. }) {
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    for _ in 0..30 {
        sequence += 1;
        prediction.tick(sequence, throttle, &[]);
    }
    prediction.tick(
        sequence + 1,
        KartInput {
            use_item: true,
            ..throttle
        },
        &[],
    );
    let kart = prediction.kart_transform();
    assert_eq!(kart.held_item, None);
    assert!(
        (kart.speed - ITEM_BOOST_FACTOR * TOP_SPEED_M_PER_S).abs() < 1e-3,
        "boosted: {} m/s",
        kart.speed
    );
}
