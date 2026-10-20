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
/// Ticks a hand keeps the item button down after pressing it: 100 ms.
const HAND_HOLD_TICKS: u32 = 6;

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
        spin_out_s: transform.spin_out_s,
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

/// What a run saw: every reconciliation's correction, in metres, and the
/// client ticks the predicted kart showed a spin-out.
struct Run {
    prediction: Prediction,
    corrections_m: Vec<f32>,
    spun_out_ticks: u32,
}

/// One kart raced for `ticks` server ticks: once the first input lands
/// the server applies the next sequence each tick, the last controls
/// standing in for one that never arrived, and snapshots every other tick;
/// the client starts its countdown when the race's first message reaches
/// it and sends one input per tick. Inputs whose sequence `lost` picks
/// never arrive. The scripted driver is the bot's driving.
fn race(ticks: u32, lost: impl Fn(u32) -> bool) -> Run {
    let mut memory = Memory::default();
    let mut item_held_ticks = 0;
    race_with(
        ticks,
        lost,
        State::new(SEED, 1, oval()),
        |prediction, _| {
            let mut input = bot::drive(
                prediction.track(),
                &[prediction.kart_transform()],
                &[prediction.standing()],
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
/// per server tick.
fn race_with(
    ticks: u32,
    lost: impl Fn(u32) -> bool,
    mut server: State,
    mut driver: impl FnMut(&Prediction, u32) -> KartInput,
    mut others: impl FnMut(u32) -> Vec<KartInput>,
) -> Run {
    let mut prediction = Prediction::new(SEED, 0, server.track().clone());
    let mut inputs_in_flight: VecDeque<(u32, u32, KartInput)> = VecDeque::new();
    let mut snapshots_in_flight: VecDeque<InFlight> = VecDeque::new();
    let mut arrived = Vec::new();
    let mut applied = (0, KartInput::default());
    let mut sequence = 0;
    let mut corrections_m = Vec::new();
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
        let input = driver(&prediction, tick);
        sequence += 1;
        prediction.tick(sequence, input, &[]);
        spun_out_ticks += u32::from(prediction.kart_transform().spin_out_s.is_some());
        if !lost(sequence) {
            inputs_in_flight.push_back((tick + 1 + ONE_WAY_TICKS, sequence, input));
        }
    }
    Run {
        prediction,
        corrections_m,
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

#[test]
fn the_prediction_takes_no_box_and_fills_its_slot_from_the_snapshots_alone() {
    // The oval with a box under slot 0: the server's kart picks it up at Go,
    // the prediction's kart stands on it through Go with an empty slot.
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    let slot = sidecar["spawn_grid"][0]["position"].clone();
    sidecar["item_boxes"] = serde_json::json!([{ "position": slot, "yaw": 0.0 }]);
    let track = Track::from_sidecar(&sidecar.to_string()).expect("the boxed oval loads");
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
    let held = server.kart_transforms()[0].held_item;
    assert!(held.is_some(), "the server's kart picked up the box");
    assert_eq!(prediction.kart_transform().held_item, None);

    // The snapshot's word fills the slot.
    prediction.reconcile(sequence, sequence, &snapshot_of(&server));
    assert_eq!(prediction.kart_transform().held_item, held);
}

#[test]
fn the_prediction_never_spins_its_kart_out_on_a_hazard_it_dropped() {
    use sim::Item;
    // The snapshot gives the kart a hazard; the press drops it behind the
    // kart and the kart drives on and backs into where it fell, which on
    // the server would spin it out. Here the hit is left to the server.
    let (mut prediction, grid) = on_the_grid();
    let mut holding = grid;
    holding.held_item = Some(Item::Hazard);
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
    assert_eq!(prediction.kart_transform().held_item, None, "fired");
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
    let start = prediction.kart_transform().position;
    for _ in 0..seconds(8.0) {
        tick(&mut prediction, reverse);
        assert_eq!(prediction.kart_transform().spin_out_s, None);
    }
    let end = prediction.kart_transform().position;
    assert!(end[2] > start[2] + 10.0, "backed well past the drop");
}

/// Full throttle toward the point the bot's lookahead down the lane at
/// `lane_x` on the oval's front straight, which runs along -Z: the pursuit
/// steer of the bot's driving, without the rest of it.
fn down_the_lane(kart: &sim::KartTransform, lane_x: f32) -> KartInput {
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
    use sim::{Item, RaceEvent};
    // Two humans: the own kart on slot 0 and, on slot 1 beside it, a kart
    // standing on a box that rolls it a hazard at Go. The seeds are tried
    // in order until one does.
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    let slot = sidecar["spawn_grid"][1]["position"].clone();
    sidecar["item_boxes"] = serde_json::json!([{ "position": slot, "yaw": 0.0 }]);
    let track = Track::from_sidecar(&sidecar.to_string()).expect("the boxed oval loads");
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
                item: Item::Hazard,
                item_box: 0,
            });
            rolled.then_some((seed, tick + 1))
        })
        .expect("a seed rolls a hazard");
    let server = State::new(seed, 2, track.clone());

    // The other kart fires its hazard the tick after it has it, trails it
    // up the lane it stands in at full throttle and drops it two seconds
    // on; the own kart waits a second after Go, then drives up that lane.
    let other = |tick: u32| {
        vec![KartInput {
            throttle: f32::from(u8::from(tick > go)),
            use_item: tick == go + 1,
            hold_item: tick > go && tick < go + seconds(2.0),
            ..KartInput::default()
        }]
    };
    let lane_x = track.spawn_grid()[1].position[0] + 0.5;
    let driver = |prediction: &Prediction, tick: u32| {
        if tick < go + seconds(1.0) {
            KartInput::default()
        } else {
            down_the_lane(&prediction.kart_transform(), lane_x)
        }
    };
    let run = race_with(seconds(9.0), |_| false, server, driver, other);

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
    // The snapshot that carried the hit reached the client a one-way trip
    // after it; the timer it brought ran the rest of the spin-out.
    let spin_out = seconds(sim::tuning::SPIN_OUT_S);
    assert!(
        (spin_out - 2 * ONE_WAY_TICKS..=spin_out).contains(&run.spun_out_ticks),
        "spun out for {} ticks, expected about {spin_out}",
        run.spun_out_ticks
    );
    let kart = run.prediction.kart_transform();
    assert_eq!(kart.spin_out_s, None, "recovered by the end");
    assert!(kart.speed > 5.0, "driving on at {} m/s", kart.speed);
}
