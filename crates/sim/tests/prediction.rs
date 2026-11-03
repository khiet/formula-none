//! Prediction of the own kart at its public seam: `Prediction::tick` with
//! each input the client sends and `Prediction::reconcile` with each
//! snapshot, against a server played here by `step` on a race of its own,
//! applying one sequence per tick as the room's race does (ADR 0019). The
//! network between them is a fixed delay each way in ticks, with the
//! inputs it loses picked by sequence. The scripted driver is the bot's
//! driving, reading the kart as the client draws it, so it hops into the
//! oval's drifts the way a player would.

mod common;

use common::{
    oval, slot_position, track, with_boxes_under, with_coins_at, with_slot_at, without_coins,
    without_item_boxes, BUMP_SPENT_S, DT, OVAL, SEED,
};

use std::collections::VecDeque;

use sim::bot::{self, Memory};
use sim::tuning::{
    INTERPOLATION_DELAY_S, RECONCILIATION_BLEND_TICKS, RECONCILIATION_TOLERANCE_M,
    RECONCILIATION_VISIBLE_CORRECTION_M, SNAPSHOTS_PER_SECOND, START_FULL_BOOST_HOLD_S,
    TICKS_PER_SECOND, TOP_SPEED_M_PER_S,
};
use sim::{
    step, BotLevel, ItemSlot, KartInput, KartReadout, Phase, Prediction, Start, State, Track,
};

const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;
/// 100 ms round trip at the tick rate, split evenly.
const ONE_WAY_TICKS: u32 = 3;
/// How far behind the newest snapshot the client draws the other karts.
const DRAWN_LAG_TICKS: u32 = (INTERPOLATION_DELAY_S * TICKS_PER_SECOND as f32) as u32;
/// Ticks a hand keeps the item button down after pressing it: 100 ms.
const HAND_HOLD_TICKS: u32 = 6;

/// A snapshot on its way to the client.
struct InFlight {
    arrives: u32,
    tick: u32,
    acknowledged: u32,
    kart: KartReadout,
}

/// What a run saw: every reconciliation's correction, in metres, and the
/// client ticks of those that moved the kart; the own kart as the client
/// drew it after each of its ticks, from the first after the one-way trip;
/// the server's karts after each of its ticks, from the grid at tick 0;
/// and the client ticks the predicted kart showed a spin-out.
struct Run {
    prediction: Prediction,
    corrections_m: Vec<f32>,
    corrected_at: Vec<u32>,
    drawn: Vec<KartReadout>,
    server: Vec<Vec<KartReadout>>,
    spun_out_ticks: u32,
}

/// The client tick the first of a run's drawn karts was drawn at.
const FIRST_DRAWN_TICK: u32 = ONE_WAY_TICKS + 1;
/// How far ahead of a client tick the server tick its input meets is: the
/// predicted kart stands where the server's will when that input lands.
const PREDICTED_LEAD_TICKS: u32 = ONE_WAY_TICKS + 1;

/// One kart raced round the oval with no coins for `ticks` server ticks,
/// as [`race_on`] races it: a coin the server's kart takes, which the
/// prediction never does, is a correction of its own.
fn race(ticks: u32, lost: impl Fn(u32) -> bool) -> Run {
    race_on(track(&without_coins(OVAL)), ticks, lost)
}

/// One kart raced on `track` for `ticks` server ticks: once the first input lands
/// the server applies the next sequence each tick, the last controls
/// standing in for one that never arrived, and snapshots every other tick;
/// the client starts its countdown when the race's first message reaches
/// it and sends one input per tick. Inputs whose sequence `lost` picks
/// never arrive. The scripted driver is the bot's driving.
fn race_on(track: Track, ticks: u32, lost: impl Fn(u32) -> bool) -> Run {
    let mut memory = Memory::default();
    let mut item_held_ticks = 0;
    race_with(
        ticks,
        lost,
        State::new(SEED, 1, track),
        |prediction, _| {
            let mut input = bot::drive(
                prediction.track(),
                BotLevel::Basic.profile(),
                &[prediction.kart_readout()],
                &[prediction.standing()],
                prediction.phase(),
                SEED,
                0,
                &mut memory,
                DT,
            );
            // A hand holds the item button past the tick it goes down, so a
            // lost press is still seen held by the server.
            if input.use_item {
                item_held_ticks = HAND_HOLD_TICKS;
            }
            input.hold_item |= item_held_ticks > 0;
            item_held_ticks = item_held_ticks.saturating_sub(1);
            input
        },
        |_| Vec::new(),
    )
}

/// [`race`] on `server`, whose first kart is the one predicted, driven by
/// `driver` from the prediction as the client draws it at each client
/// tick; the server's other humans are driven by `others`, one input each
/// per server tick. The prediction meets no other kart: the client draws
/// none.
fn race_with(
    ticks: u32,
    lost: impl Fn(u32) -> bool,
    server: State,
    driver: impl FnMut(&Prediction, u32) -> KartInput,
    others: impl FnMut(u32) -> Vec<KartInput>,
) -> Run {
    race_drawing(ticks, lost, server, driver, others, false)
}

/// [`race_with`], and when `draws_others` the client draws the other karts
/// where the server had them the interpolation delay before its newest
/// snapshot, so the prediction meets them there.
fn race_drawing(
    ticks: u32,
    lost: impl Fn(u32) -> bool,
    mut server: State,
    mut driver: impl FnMut(&Prediction, u32) -> KartInput,
    mut others: impl FnMut(u32) -> Vec<KartInput>,
    draws_others: bool,
) -> Run {
    let mut prediction = Prediction::new(SEED, 0, server.track().clone());
    let mut history = vec![server.kart_readouts()];
    let mut inputs_in_flight: VecDeque<(u32, u32, KartInput)> = VecDeque::new();
    let mut snapshots_in_flight: VecDeque<InFlight> = VecDeque::new();
    let mut arrived = Vec::new();
    let mut applied = (0, KartInput::default());
    let mut sequence = 0;
    let mut corrections_m = Vec::new();
    let mut corrected_at = Vec::new();
    let mut drawn = Vec::new();
    let mut spun_out_ticks = 0;
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
        let mut inputs = vec![applied.1];
        inputs.extend(others(tick));
        server = step(server, &inputs, DT);
        history.push(server.kart_readouts());
        if tick.is_multiple_of(TICKS_PER_SNAPSHOT) {
            snapshots_in_flight.push_back(InFlight {
                arrives: tick + ONE_WAY_TICKS,
                tick,
                acknowledged: applied.0,
                kart: server.kart_readouts()[0],
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
            let moved = prediction.reconcile(snapshot.tick, snapshot.acknowledged, &snapshot.kart);
            if moved > RECONCILIATION_TOLERANCE_M {
                corrected_at.push(tick);
            }
            corrections_m.push(moved);
        }
        let input = driver(&prediction, tick);
        sequence += 1;
        let karts = if draws_others {
            let seen = (tick - ONE_WAY_TICKS).saturating_sub(DRAWN_LAG_TICKS);
            history[seen as usize].as_slice()
        } else {
            &[]
        };
        prediction.tick(sequence, input, karts);
        drawn.push(prediction.kart_readout());
        spun_out_ticks += u32::from(prediction.kart_readout().spin_out_s.is_some());
        if !lost(sequence) {
            inputs_in_flight.push_back((tick + 1 + ONE_WAY_TICKS, sequence, input));
        }
    }
    Run {
        prediction,
        corrections_m,
        corrected_at,
        drawn,
        server: history,
        spun_out_ticks,
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

#[test]
fn a_coin_the_servers_kart_takes_is_corrected_by_less_than_a_player_can_see() {
    let run = race_on(oval(), seconds(20.0), |_| false);
    let taken = run.server.last().expect("the server raced")[0].coins;
    assert!(taken > 0, "the server's kart took a coin");
    assert!(run.prediction.corrections() > 0, "the prediction took none");
    let largest = run.corrections_m.iter().copied().fold(0.0, f32::max);
    assert!(
        largest < RECONCILIATION_VISIBLE_CORRECTION_M,
        "moved {largest} m"
    );
}

/// A prediction on the grid, its countdown under way and its inputs
/// acknowledged by a snapshot that agreed.
fn on_the_grid() -> (Prediction, KartReadout) {
    let mut prediction = Prediction::new(SEED, 0, oval());
    for sequence in 1..=10 {
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    let grid = State::new(SEED, 1, oval()).kart_readouts()[0];
    prediction.reconcile(10, 4, &grid);
    assert!(matches!(prediction.phase(), Phase::Countdown { .. }));
    (prediction, grid)
}

#[test]
fn a_correction_is_blended_from_where_the_kart_was_drawn_over_the_tuned_ticks() {
    let (mut prediction, grid) = on_the_grid();
    let drawn = prediction.kart_readout().position;
    let mut aside = grid;
    aside.position[0] += 1.0;

    let moved = prediction.reconcile(12, 5, &aside);
    assert!((moved - 1.0).abs() < 1e-4, "moved {moved} m");
    assert_eq!(prediction.corrections(), 1);
    assert_eq!(prediction.kart_readout().position, drawn, "no jump");
    let mut sequence = 10;
    for tick in 1..=RECONCILIATION_BLEND_TICKS {
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
        let x = prediction.kart_readout().position[0];
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
    assert_eq!(prediction.kart_readout().item_slot, ItemSlot::Empty);

    // The server rolled the kart a boost from a box the prediction did not
    // see taken; the kart is where it was predicted, so nothing moves.
    let mut holding = grid;
    holding.item_slot = ItemSlot::Held(Item::Boost);
    assert_eq!(prediction.reconcile(12, 5, &holding), 0.0);
    assert_eq!(prediction.corrections(), 0);
    assert_eq!(
        prediction.kart_readout().item_slot,
        ItemSlot::Held(Item::Boost)
    );

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
    let kart = prediction.kart_readout();
    assert_eq!(kart.item_slot, ItemSlot::Empty);
    assert!(
        (kart.speed - ITEM_BOOST_FACTOR * TOP_SPEED_M_PER_S).abs() < 1e-3,
        "boosted: {} m/s",
        kart.speed
    );
}

#[test]
fn the_snapshots_bump_is_taken_without_a_correction() {
    let (mut prediction, grid) = on_the_grid();
    assert_eq!(prediction.kart_readout().bump, None);

    // The server's kart was nudged where it stands and carries the bump;
    // the kart is where it was predicted, so nothing moves, but the bump is
    // the snapshot's word and the prediction takes it in place of its own.
    let mut nudged = grid;
    nudged.bump = Some([2.0, 0.0, 0.0]);
    assert_eq!(prediction.reconcile(12, 5, &nudged), 0.0);
    assert_eq!(prediction.corrections(), 0);
    assert_eq!(prediction.kart_readout().bump, Some([2.0, 0.0, 0.0]));
}

#[test]
fn the_prediction_takes_no_box_and_fills_its_slot_from_the_snapshots_alone() {
    // The oval with a box under slot 0: the server's kart picks it up at Go,
    // the prediction's kart stands on it through Go with an empty slot.
    let track = track(&with_boxes_under(&without_item_boxes(OVAL), [0]));
    let mut server = State::new(SEED, 1, track.clone());
    let mut prediction = Prediction::new(SEED, 0, track);
    let mut sequence = 0;
    while !matches!(server.phase(), Phase::Racing { .. }) {
        server = step(server, &[KartInput::default()], DT);
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    for _ in 0..3 {
        server = step(server, &[KartInput::default()], DT);
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    let held = server.kart_readouts()[0].item_slot;
    assert!(held.item().is_some(), "the server's kart picked up the box");
    assert_eq!(prediction.kart_readout().item_slot, ItemSlot::Empty);

    // The snapshot's word fills the slot.
    prediction.reconcile(sequence, sequence, &server.kart_readouts()[0]);
    assert_eq!(prediction.kart_readout().item_slot, held);

    // From there the prediction runs the roll down as the server does:
    // the snapshots through the landing agree with it, and it lands the
    // item itself.
    let corrections = prediction.corrections();
    let ItemSlot::Rolling { item, .. } = held else {
        panic!("the item rolls first: {held:?}");
    };
    while prediction.kart_readout().item_slot != ItemSlot::Held(item) {
        server = step(server, &[KartInput::default()], DT);
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
        prediction.reconcile(sequence, sequence, &server.kart_readouts()[0]);
        assert!(
            sequence < 1_000,
            "the prediction's roll never landed: {:?}",
            prediction.kart_readout().item_slot
        );
    }
    assert_eq!(server.kart_readouts()[0].item_slot, ItemSlot::Held(item));
    assert_eq!(
        prediction.corrections(),
        corrections,
        "no correction through the roll"
    );
}

#[test]
fn the_prediction_takes_no_coin_and_adopts_the_snapshots_count() {
    // The oval with a coin under slot 0: the server's kart takes it at Go,
    // the prediction's kart stands on it through Go and takes nothing.
    let sidecar = with_coins_at(&without_coins(OVAL), [slot_position(OVAL, 0)]);
    let track = track(&sidecar);
    let mut server = State::new(SEED, 1, track.clone());
    let mut prediction = Prediction::new(SEED, 0, track);
    let mut sequence = 0;
    while !matches!(server.phase(), Phase::Racing { .. }) {
        server = step(server, &[KartInput::default()], DT);
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    for _ in 0..3 {
        server = step(server, &[KartInput::default()], DT);
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    assert_eq!(
        server.kart_readouts()[0].coins,
        1,
        "the server's kart took it"
    );
    assert_eq!(prediction.kart_readout().coins, 0);

    // The snapshot's word is the count, kart standing where it was.
    prediction.reconcile(sequence, sequence, &server.kart_readouts()[0]);
    assert_eq!(prediction.kart_readout().coins, 1);
    for _ in 0..3 {
        sequence += 1;
        prediction.tick(sequence, KartInput::default(), &[]);
    }
    assert_eq!(prediction.kart_readout().coins, 1, "and keeps it");
}

#[test]
fn the_prediction_never_spins_its_kart_out_on_a_hazard_it_dropped() {
    use sim::Item;
    // The snapshot gives the kart a hazard; the press drops it behind the
    // kart and the kart drives on and backs into where it fell, which on
    // the server would spin it out. Here the hit is left to the server.
    let (mut prediction, grid) = on_the_grid();
    let mut holding = grid;
    holding.item_slot = ItemSlot::Held(Item::Hazard);
    prediction.reconcile(12, 5, &holding);
    let mut sequence = 10;
    let mut tick = |prediction: &mut Prediction, input: KartInput| {
        sequence += 1;
        prediction.tick(sequence, input, &[]);
    };
    while !matches!(prediction.phase(), Phase::Racing { .. }) {
        tick(&mut prediction, KartInput::default());
    }
    tick(
        &mut prediction,
        KartInput {
            use_item: true,
            ..KartInput::default()
        },
    );
    assert_eq!(
        prediction.kart_readout().item_slot,
        ItemSlot::Empty,
        "fired"
    );
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    for _ in 0..seconds(1.5) {
        tick(&mut prediction, throttle);
    }
    let reverse = KartInput {
        brake: 1.0,
        ..KartInput::default()
    };
    let start = prediction.kart_readout().position;
    for _ in 0..seconds(8.0) {
        tick(&mut prediction, reverse);
        assert_eq!(prediction.kart_readout().spin_out_s, None);
    }
    let end = prediction.kart_readout().position;
    assert!(end[2] > start[2] + 10.0, "backed well past the drop");
}

/// Full throttle toward the point the bot's lookahead ahead in the lane at
/// `lane_x` on the oval's front straight, which runs along -Z: the pursuit
/// steer of the bot's driving, without the rest of it.
fn down_the_lane(kart: &sim::KartReadout, lane_x: f32) -> KartInput {
    use sim::tuning::BOT_LOOKAHEAD_M;
    let [x, y, z, w] = kart.rotation;
    let yaw = (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z));
    let (dx, dz) = (lane_x - kart.position[0], -BOT_LOOKAHEAD_M);
    let distance = (dx * dx + dz * dz).sqrt();
    let (sin, cos) = yaw.sin_cos();
    let rightward = (dx * cos - dz * sin) / distance;
    KartInput {
        steer: (rightward * 4.0).clamp(-1.0, 1.0),
        throttle: 1.0,
        ..KartInput::default()
    }
}

#[test]
fn a_hit_the_server_decides_reaches_the_prediction_as_one_correction_and_the_spin_out_holds_it() {
    use sim::tuning::ITEM_ROLL_S;
    use sim::{Item, RaceEvent};
    // Two humans: the own kart on slot 0 and, on slot 1 beside it, a kart
    // standing on a box that rolls it a hazard at Go. The seeds are tried
    // in order until one does.
    let track = track(&with_boxes_under(&without_item_boxes(OVAL), [1]));
    let idle = [KartInput::default(); 2];
    let (seed, go) = (0..)
        .find_map(|seed| {
            let mut state = State::new(seed, 2, track.clone());
            let mut tick = 0;
            while !matches!(state.phase(), Phase::Racing { .. }) {
                state = step(state, &idle, DT);
                tick += 1;
            }
            let state = step(state, &idle, DT);
            let rolled = state.events().contains(&RaceEvent::ItemPicked {
                kart: 1,
                item: Some(Item::Hazard),
                item_box: 0,
            });
            rolled.then_some((seed, tick + 1))
        })
        .expect("a seed rolls a hazard");
    let server = State::new(seed, 2, track.clone());
    // A tick past the hazard landing in the other kart's slot.
    let has = go + seconds(ITEM_ROLL_S) + 2;

    // The other kart fires its hazard the tick after it has it, trails it
    // up the lane it stands in at full throttle and drops it two seconds
    // on; the own kart waits a second after that, then drives up that lane.
    let other = |tick: u32| {
        vec![KartInput {
            throttle: f32::from(u8::from(tick > has)),
            use_item: tick == has + 1,
            hold_item: tick > has && tick < has + seconds(2.0),
            ..KartInput::default()
        }]
    };
    let lane_x = track.spawn_grid()[1].position[0] + 0.5;
    let driver = |prediction: &Prediction, tick: u32| {
        if tick < has + seconds(1.0) {
            KartInput::default()
        } else {
            down_the_lane(&prediction.kart_readout(), lane_x)
        }
    };
    let run = race_with(has + seconds(6.0), |_| false, server, driver, other);

    assert_eq!(
        run.prediction.corrections(),
        1,
        "the hit is the one correction: {:?}",
        run.corrections_m
    );
    let hit = run
        .corrections_m
        .iter()
        .position(|&moved| moved > RECONCILIATION_TOLERANCE_M)
        .expect("the hit corrected the kart");
    assert!(
        run.corrections_m[hit..]
            .iter()
            .skip(1)
            .all(|&moved| moved <= RECONCILIATION_TOLERANCE_M),
        "every snapshot after the hit agreed: {:?}",
        &run.corrections_m[hit..]
    );
    // The first snapshot after the hit, up to a snapshot interval later,
    // reached the client a one-way trip after it; the timer it brought ran
    // the rest of the spin-out.
    let spin_out = seconds(sim::tuning::SPIN_OUT_S);
    let late = 2 * ONE_WAY_TICKS + TICKS_PER_SNAPSHOT - 1;
    assert!(
        (spin_out - late..=spin_out).contains(&run.spun_out_ticks),
        "spun out for {} ticks, expected about {spin_out}",
        run.spun_out_ticks
    );
    let kart = run.prediction.kart_readout();
    assert_eq!(kart.spin_out_s, None, "recovered by the end");
    assert!(kart.speed > 5.0, "driving on at {} m/s", kart.speed);
}

/// [`race_with`] for one kart whose driver holds the throttle from the
/// count reaching the full boost's edge through Go and on, with the input
/// that began the hold lost when `lose_the_press`: the server then sees
/// the hold begin a tick late, on the weak side of the edge.
fn rocket_start_at_the_full_edge(lose_the_press: bool) -> Run {
    use std::cell::Cell;
    let press_sequence = Cell::new(None);
    let driver = |prediction: &Prediction, tick: u32| {
        let pressed = match prediction.phase() {
            Phase::Countdown { remaining_s } => remaining_s <= START_FULL_BOOST_HOLD_S + DT / 2.0,
            _ => true,
        };
        if pressed && press_sequence.get().is_none() {
            // The sequence `race_with` sends this input under: one per
            // tick from the first after the one-way trip.
            press_sequence.set(Some(tick - ONE_WAY_TICKS));
        }
        KartInput {
            throttle: f32::from(u8::from(pressed)),
            ..KartInput::default()
        }
    };
    let lost = |sequence: u32| lose_the_press && press_sequence.get() == Some(sequence);
    race_with(
        seconds(sim::tuning::COUNTDOWN_S + 2.0),
        lost,
        State::new(SEED, 1, oval()),
        driver,
        |_| Vec::new(),
    )
}

#[test]
fn a_hold_at_a_window_edge_the_server_decides_differently_is_corrected_once_to_its_verdict() {
    // Nothing lost: the server reads the hold the client predicted.
    let run = rocket_start_at_the_full_edge(false);
    assert_eq!(run.prediction.kart_readout().start, Start::FullBoost);
    assert_eq!(run.prediction.corrections(), 0, "{:?}", run.corrections_m);

    // The press lost: the server's hold is a tick short of the full edge,
    // so its verdict is the weak boost. The verdict reaches the prediction
    // as one correction, and the kart runs the weak boost from there.
    let run = rocket_start_at_the_full_edge(true);
    let kart = run.prediction.kart_readout();
    assert_eq!(kart.start, Start::WeakBoost);
    assert_eq!(
        run.prediction.corrections(),
        1,
        "the verdict is the one correction: {:?}",
        run.corrections_m
    );
    let verdict = run
        .corrections_m
        .iter()
        .position(|&moved| moved > RECONCILIATION_TOLERANCE_M)
        .expect("the verdict corrected the kart");
    assert!(
        run.corrections_m[verdict..]
            .iter()
            .skip(1)
            .all(|&moved| moved <= RECONCILIATION_TOLERANCE_M),
        "every snapshot after the verdict agreed: {:?}",
        &run.corrections_m[verdict..]
    );
    assert!(
        kart.speed <= TOP_SPEED_M_PER_S,
        "the weak boost is spent by the end: {} m/s",
        kart.speed
    );
}

#[test]
fn a_finished_kart_is_predicted_driven_by_its_bot_not_by_the_players_inputs() {
    // The other human waits on the grid, holding the race open; the player
    // laps with the bot's driving, then reverses hard on full lock. No item
    // boxes: the bot's items decide whether it meets its own hazard, which
    // this test does not test.
    let mut memory = Memory::default();
    let mut finished_at = None;
    let mut corrections_at_the_line = 0;
    let run = race_with(
        seconds(90.0),
        |_| false,
        State::new(SEED, 2, track(&without_item_boxes(OVAL))),
        |prediction, tick| {
            if prediction.standing().finish_time_s.is_some() {
                if finished_at.is_none() {
                    finished_at = Some(tick);
                    corrections_at_the_line = prediction.corrections();
                }
                return KartInput {
                    brake: 1.0,
                    steer: 1.0,
                    ..KartInput::default()
                };
            }
            bot::drive(
                prediction.track(),
                BotLevel::Basic.profile(),
                &[prediction.kart_readout()],
                &[prediction.standing()],
                prediction.phase(),
                SEED,
                0,
                &mut memory,
                DT,
            )
        },
        |_| vec![KartInput::default()],
    );
    let finished_at = finished_at.expect("the kart finished");
    assert!(
        seconds(90.0) - finished_at > seconds(10.0),
        "ten seconds past the line"
    );
    let kart = run.prediction.kart_readout();
    assert!(
        kart.speed > 0.5 * TOP_SPEED_M_PER_S,
        "the bot drives on at {} m/s",
        kart.speed
    );
    assert_eq!(
        run.prediction.corrections(),
        corrections_at_the_line,
        "the bot's driving past the line is not counted against the player"
    );
}

#[test]
fn a_touch_the_server_places_elsewhere_corrects_the_kart_while_the_bump_lasts_and_no_longer() {
    // The own kart idles on the oval's front straight from Go; the other
    // human, 10 m behind in its column, drives into its back at full
    // throttle and brakes once it has hit. The client draws the other kart
    // the interpolation delay and a one-way trip behind the server, so the
    // prediction's touch comes later than the server's: the bump the
    // server's snapshot carries is not the one the prediction gave itself.
    let sidecar = with_slot_at(OVAL, 0, [28.0, 0.0, -6.0], 0.0);
    let track = track(&with_slot_at(&sidecar, 1, [28.0, 0.0, 4.0], 0.0));
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let brake = KartInput {
        brake: 1.0,
        ..KartInput::default()
    };
    // The server's own tick of the touch: the first the own kart carries a
    // bump. The run below steps the server the same way, so it touches on
    // the same tick.
    let mut rehearsal = State::new(SEED, 2, track.clone());
    let mut hit = 0;
    while rehearsal.kart_readouts()[0].bump.is_none() {
        rehearsal = step(rehearsal, &[KartInput::default(), throttle], DT);
        hit += 1;
        assert!(hit < seconds(10.0), "the other kart hits the own kart");
    }
    let server = State::new(SEED, 2, track);
    let other = |tick: u32| vec![if tick <= hit { throttle } else { brake }];
    let ticks = hit + seconds(3.0);
    let run = race_drawing(
        ticks,
        |_| false,
        server,
        |_, _| KartInput::default(),
        other,
        true,
    );

    let first = run
        .corrections_m
        .iter()
        .position(|&moved| moved > RECONCILIATION_TOLERANCE_M)
        .expect("the touch corrected the kart");
    // Snapshots come every other tick; the bump is spent this many on
    // from the one that brought it, with the lag the client draws the other
    // kart at on top.
    let lasts = (seconds(BUMP_SPENT_S) + ONE_WAY_TICKS + DRAWN_LAG_TICKS) as usize
        / TICKS_PER_SNAPSHOT as usize;
    assert!(
        run.corrections_m.len() > first + lasts + 1,
        "the run outlasts the bump"
    );
    assert!(
        run.corrections_m[first + lasts..]
            .iter()
            .all(|&moved| moved <= RECONCILIATION_TOLERANCE_M),
        "every snapshot once the bump is spent agreed: {:?}",
        &run.corrections_m[first + lasts..]
    );
    // The push the server made before the client could see the other kart
    // reach it is more than a tick of travel, yet the drawn kart never
    // covers that much in a tick: the correction is blended out.
    let tick_of_travel = TOP_SPEED_M_PER_S * DT;
    let moved = run.corrections_m[first];
    assert!(moved > tick_of_travel, "the touch moved the kart {moved} m");
    let distance = |a: [f32; 3], b: [f32; 3]| {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    let furthest = run
        .drawn
        .windows(2)
        .map(|pair| distance(pair[0].position, pair[1].position))
        .fold(0.0, f32::max);
    assert!(
        furthest < tick_of_travel,
        "the drawn kart never jumps: {furthest} m in a tick"
    );
    // And the blend ticks after the correction the drawn kart is where
    // the server has its kart the predicted lead ahead, and stays there.
    let corrected = run.corrected_at[0];
    for tick in corrected + RECONCILIATION_BLEND_TICKS..=ticks - PREDICTED_LEAD_TICKS {
        let drawn = run.drawn[(tick - FIRST_DRAWN_TICK) as usize].position;
        let server = run.server[(tick + PREDICTED_LEAD_TICKS) as usize][0].position;
        let off = distance(drawn, server);
        assert!(
            off < 0.01,
            "tick {tick}, {} after the correction: drawn {off} m from the server's kart",
            tick - corrected
        );
    }
    // Hit from behind while idling, the own kart is carried by the bump
    // and keeps its own speed of nothing, as the server has it; the bump
    // is never read as driving.
    let fastest = run
        .drawn
        .iter()
        .map(|kart| kart.speed.abs())
        .fold(0.0, f32::max);
    assert!(
        fastest < 0.5,
        "the own kart's speed stays its own: {fastest} m/s"
    );
    let pushed = run.drawn.iter().any(|kart| kart.velocity[2] < -1.0);
    assert!(pushed, "the own kart was pushed down the straight");
}

#[test]
fn a_kart_alone_never_charges_even_with_a_kart_drawn_ahead_of_it() {
    use sim::Slipstream;
    // The oval raced alone: nothing to charge off.
    let run = race(seconds(5.0), |_| false);
    assert!(run.drawn.iter().all(|kart| kart.slipstream.is_none()));

    // The client draws another kart 6 m ahead of the own kart every tick,
    // which the prediction stands in its race as a kinematic body; the
    // wake is the server's to judge, so the own kart never charges off it.
    let course = track(&without_coins(&without_item_boxes(OVAL)));
    let mut prediction = Prediction::new(SEED, 0, course.clone());
    let mut drawn = common::started(State::new(SEED, 2, course)).kart_readouts();
    let countdown = common::ticks(sim::tuning::COUNTDOWN_S) as u32;
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    for sequence in 1..=countdown + seconds(4.0) {
        let input = if sequence <= countdown {
            KartInput::default()
        } else {
            throttle
        };
        let own = prediction.kart_readout();
        drawn[0] = own;
        drawn[1].position = [own.position[0], own.position[1], own.position[2] - 6.0];
        drawn[1].rotation = own.rotation;
        drawn[1].velocity = own.velocity;
        prediction.tick(sequence, input, &drawn);
        assert_eq!(prediction.kart_readout().slipstream, None::<Slipstream>);
    }
    assert_eq!(prediction.kart_readout().speed, TOP_SPEED_M_PER_S);
}

#[test]
fn the_snapshots_slipstream_is_adopted_and_the_tow_runs_down_between_snapshots_without_a_correction(
) {
    use sim::tuning::{SLIPSTREAM_TOP_SPEED_FACTOR, SLIPSTREAM_TOW_S};
    use sim::Slipstream;
    // Two humans down the oval's front straight, the leader 6 m ahead of
    // the predicted kart, both idle through the
    // countdown and at full throttle from Go, so the gap holds until the
    // tow takes. A little after it has, the leader pulls right a lane and
    // straightens, leaving the predicted kart's tow to run down as it
    // passes beside.
    // The lane at x = 25 is clear of the tyre wall by the turn's entry,
    // which the run stops short of.
    let sidecar = with_slot_at(
        &without_coins(&without_item_boxes(OVAL)),
        0,
        [25.0, 0.0, 4.0],
        0.0,
    );
    let sidecar = with_slot_at(&sidecar, 1, [25.0, 0.0, -2.0], 0.0);
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let countdown = common::ticks(sim::tuning::COUNTDOWN_S) as u32;
    let run = race_with(
        countdown + seconds(4.4),
        |_| false,
        State::new(SEED, 2, track(&sidecar)),
        |prediction, _| match prediction.phase() {
            Phase::Countdown { .. } => KartInput::default(),
            _ => throttle,
        },
        |tick| {
            if tick <= countdown {
                return vec![KartInput::default()];
            }
            let steer = match tick - countdown {
                156..162 => 1.0,
                186..192 => -1.0,
                _ => 0.0,
            };
            vec![KartInput { steer, ..throttle }]
        },
    );

    assert!(
        run.corrections_m
            .iter()
            .all(|&moved| moved <= RECONCILIATION_TOLERANCE_M),
        "corrections of {:?} m",
        run.corrections_m
            .iter()
            .filter(|&&moved| moved > RECONCILIATION_TOLERANCE_M)
            .collect::<Vec<_>>()
    );
    let slipstreams: Vec<Option<Slipstream>> =
        run.drawn.iter().map(|kart| kart.slipstream).collect();
    assert!(slipstreams
        .iter()
        .any(|slipstream| matches!(slipstream, Some(Slipstream::Charging { .. }))));
    let fastest = run
        .drawn
        .iter()
        .map(|kart| kart.speed)
        .fold(0.0_f32, f32::max);
    assert_eq!(fastest, TOP_SPEED_M_PER_S * SLIPSTREAM_TOP_SPEED_FACTOR);

    // From the first drawn tick with the tow running down, every drawn
    // tick runs it down by one tick until it is gone, with no snapshot
    // putting it back.
    let running_down = slipstreams
        .iter()
        .position(|slipstream| {
            matches!(slipstream, Some(Slipstream::Tow { left_s }) if *left_s < SLIPSTREAM_TOW_S)
        })
        .expect("the tow ran down");
    let mut expected = match slipstreams[running_down] {
        Some(Slipstream::Tow { left_s }) => left_s,
        _ => unreachable!(),
    };
    let mut ran_down_for = 1;
    for slipstream in &slipstreams[running_down + 1..] {
        expected -= DT;
        match slipstream {
            Some(Slipstream::Tow { left_s }) => {
                assert!(
                    (left_s - expected).abs() < 1e-4,
                    "{left_s} s, expected {expected}"
                );
                ran_down_for += 1;
            }
            None => break,
            other => panic!("{other:?} while the tow ran down"),
        }
    }
    assert!(ran_down_for > 30, "ran down for {ran_down_for} ticks");
    assert!(
        slipstreams.last().unwrap().is_none(),
        "the tow is gone by the end"
    );
}
