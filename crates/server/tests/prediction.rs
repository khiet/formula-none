//! Prediction of the own kart against the authoritative server (ADR 0009),
//! through the fake transport at the round trip and loss the ADR targets.
//! The client here is the browser's tick loop in Rust: it reconciles each
//! snapshot as it arrives and hands it to the interpolation of the other
//! karts, then predicts the tick with the input it sends against those
//! karts where it draws them. The scripted driver is the bot's driving,
//! reading the kart as the client draws it, so the run hops into the oval's
//! drifts.

mod common;

use std::time::Duration;

use common::{projectiles, race_together, read, snapshot_kart, wire, with_rocket_start};
use protocol::{ClientMessage, ServerMessage};
use server::fake_transport::{Conditions, Harness, TRACK};
use sim::bot::{self, Memory};
use sim::tuning::{
    COUNTDOWN_S, KART_HALF_LENGTH_M, RECONCILIATION_VISIBLE_CORRECTION_M, TICKS_PER_SECOND,
};
use sim::{Interpolation, KartInput, Prediction, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);
/// Ticks a hand keeps the item button down after pressing it: 100 ms.
const HAND_HOLD_TICKS: u32 = 6;
/// The one-way delay of the ADR 0009 target: 100 ms round trip.
const ONE_WAY: Duration = Duration::from_millis(50);
/// Seconds after a spin-out during which contact with karts drawn 100 ms
/// behind may still correct the restarting kart.
const RESTART_IN_THE_PACK_S: f32 = 3.0;
/// Ticks after a hit's snapshot during which a correction is the hit's: the
/// spin-out and the restart in the pack it leaves the kart in.
const HIT_AFTERMATH_TICKS: u32 =
    ((sim::tuning::SPIN_OUT_S + RESTART_IN_THE_PACK_S) * TICKS_PER_SECOND as f32) as u32;

#[tokio::test(start_paused = true)]
async fn at_a_100_ms_round_trip_and_2_percent_loss_the_own_kart_is_corrected_out_of_sight() {
    let mut harness = Harness::new(Conditions {
        delay: ONE_WAY,
        jitter: Duration::ZERO,
        loss: 0.02,
        seed: 18,
    });
    let code = harness.create_room();
    let mut player = harness.connect();
    let (slot, _) = player.join(&code, "driver").await.unwrap();
    player.send(&ClientMessage::Ready { ready: true });
    player.next_room().await.unwrap();
    player.send(&ClientMessage::Start);
    let countdown = player.next_countdown().await.unwrap();
    assert_eq!(countdown.track, TRACK);
    let kart = countdown
        .grid
        .iter()
        .position(|&seat| seat == slot)
        .expect("the player is on the grid");

    let mut interpolation = Interpolation::new();
    let mut prediction = Prediction::new(
        countdown.seed,
        kart,
        Track::from_sidecar(OVAL).expect("the oval fixture loads"),
    );
    let mut memory = Memory::default();
    // A hand holds the item button past the tick it goes down; a lost
    // press is then still seen held by the server.
    let mut item_held_ticks = 0;
    let mut ticks = tokio::time::interval(TICK);
    let mut sequence = 0;
    let mut corrections_m = Vec::new();
    let mut hits_m = Vec::new();
    let mut hit_tick = None;
    let mut newest_tick = 0;
    // Twenty seconds of racing: both of the oval's turns, each lap.
    let end_tick = ((COUNTDOWN_S + 20.0) * TICKS_PER_SECOND as f32) as u32;
    while newest_tick < end_tick {
        ticks.tick().await;
        while let Some(message) = player.try_recv() {
            if let ServerMessage::Snapshot(snapshot) = message {
                newest_tick = newest_tick.max(snapshot.tick);
                interpolation.push(
                    snapshot.tick,
                    snapshot.karts.iter().map(snapshot_kart).collect(),
                    projectiles(&snapshot),
                );
                // A hit is the server's alone: the snapshot that brings its
                // spin-out corrects the kart by however far it was predicted
                // on past the hazard, and the restart that follows meets
                // karts drawn 100 ms behind, a contact ADR 0009 leaves
                // uncompensated. Neither correction is the loss's.
                if snapshot.karts[kart].spin_out_s.is_some()
                    && prediction.kart_transform().spin_out_s.is_none()
                {
                    hit_tick = Some(snapshot.tick);
                }
                let moved = prediction.reconcile(
                    snapshot.tick,
                    snapshot.acknowledged[kart],
                    &snapshot_kart(&snapshot.karts[kart]),
                );
                let after_a_hit =
                    hit_tick.is_some_and(|hit| snapshot.tick <= hit + HIT_AFTERMATH_TICKS);
                if after_a_hit {
                    hits_m.push(moved);
                } else {
                    corrections_m.push(moved);
                }
            }
        }
        let mut input = with_rocket_start(
            prediction.phase(),
            bot::drive(
                prediction.track(),
                &[prediction.kart_transform()],
                &[prediction.standing()],
                0,
                &mut memory,
                TICK.as_secs_f32(),
            ),
        );
        if input.use_item {
            item_held_ticks = HAND_HOLD_TICKS;
        }
        input.hold_item |= item_held_ticks > 0;
        item_held_ticks = item_held_ticks.saturating_sub(1);
        sequence += 1;
        let input = wire(sequence, input);
        interpolation.advance();
        prediction.tick(sequence, read(input), &interpolation.kart_transforms());
        player.send_input(input);
    }

    assert!(
        prediction.standing().spline_progress > 100.0 || prediction.standing().laps_completed > 0,
        "the kart got round: {:?}",
        prediction.standing()
    );
    assert!(
        corrections_m.len() > 20 * 30 / 2,
        "{} snapshots measured",
        corrections_m.len()
    );
    let largest = corrections_m.iter().copied().fold(0.0, f32::max);
    assert!(
        largest < RECONCILIATION_VISIBLE_CORRECTION_M,
        "corrected by {largest} m"
    );
    assert!(
        corrections_m.iter().any(|&moved| moved > 0.0),
        "the loss corrected nothing, so it never reached the prediction"
    );
    // A hit moves the kart back by at most the round trip's travel.
    let round_trip_m = 2.0 * ONE_WAY.as_secs_f32() * sim::tuning::TOP_SPEED_M_PER_S;
    assert!(
        hits_m.iter().all(|&moved| moved <= round_trip_m),
        "a hit corrected by more than the round trip: {hits_m:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn driving_into_another_kart_never_predicts_the_own_kart_visibly_past_where_the_server_puts_it(
) {
    // A seed whose race rolls no bot a rocket start, so the one contact is
    // the one driven into; the first snapshots before Go check that.
    let mut harness = Harness::new(Conditions {
        delay: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
        seed: 76,
    });
    let code = harness.create_room();
    // Three humans: the oval's grid stands the third directly behind the
    // first, which never drives, and the second beside them.
    let mut players = [harness.connect(), harness.connect(), harness.connect()];
    let countdown = race_together(&code, &mut players).await.remove(2);
    let [_parked, _beside, mut driver] = players;
    let (parked, kart) = (0, 2);

    let mut interpolation = Interpolation::new();
    let mut prediction = Prediction::new(
        countdown.seed,
        kart,
        Track::from_sidecar(OVAL).expect("the oval fixture loads"),
    );
    // Idle through the countdown, then straight down the grid at full
    // throttle, into the parked kart.
    let flat_out = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let mut ticks = tokio::time::interval(TICK);
    let mut sequence = 0;
    let lap_m = prediction.track().spline_length();
    // The spline progress each sequence was predicted to, unblended.
    let mut predicted = vec![0.0];
    let mut overruns_m = Vec::new();
    let mut closest_m = f32::MAX;
    let mut newest_tick = 0;
    let end_tick = ((COUNTDOWN_S + 3.0) * TICKS_PER_SECOND as f32) as u32;
    while newest_tick < end_tick {
        ticks.tick().await;
        while let Some(message) = driver.try_recv() {
            if let ServerMessage::Snapshot(snapshot) = message {
                newest_tick = newest_tick.max(snapshot.tick);
                if (COUNTDOWN_S - 0.2..COUNTDOWN_S).contains(&common::seconds(snapshot.tick)) {
                    assert!(
                        snapshot
                            .karts
                            .iter()
                            .all(|kart| kart.start == protocol::Start::Clean),
                        "a bot rolled a rocket start; the seed must roll none"
                    );
                }
                let [_, _, own_z] = snapshot.karts[kart].position;
                let [_, _, parked_z] = snapshot.karts[parked].position;
                closest_m = closest_m.min(own_z - parked_z);
                let acknowledged = snapshot.acknowledged[kart];
                if acknowledged > 0 {
                    // Progress wraps at the start line; the shorter way
                    // round is the overrun.
                    let ahead_m =
                        predicted[acknowledged as usize] - snapshot.karts[kart].spline_progress;
                    overruns_m.push((ahead_m + lap_m / 2.0).rem_euclid(lap_m) - lap_m / 2.0);
                }
                interpolation.push(
                    snapshot.tick,
                    snapshot.karts.iter().map(snapshot_kart).collect(),
                    projectiles(&snapshot),
                );
                prediction.reconcile(
                    snapshot.tick,
                    acknowledged,
                    &snapshot_kart(&snapshot.karts[kart]),
                );
            }
        }
        sequence += 1;
        let input = wire(
            sequence,
            if matches!(prediction.phase(), sim::Phase::Racing { .. }) {
                flat_out
            } else {
                KartInput::default()
            },
        );
        interpolation.advance();
        prediction.tick(sequence, read(input), &interpolation.kart_transforms());
        driver.send_input(input);
        predicted.push(prediction.standing().spline_progress);
    }

    assert!(
        closest_m < 2.0 * KART_HALF_LENGTH_M + 0.1,
        "the karts never touched: {closest_m} m apart at the closest"
    );
    let overrun = overruns_m.iter().copied().fold(f32::MIN, f32::max);
    // Once pushed, the other kart is drawn 100 ms behind where the server
    // has it, overlapping the own kart, which the contact pushes out ahead
    // of it; with no lag compensation (ADR 0009) that leaves a few
    // centimetres to correct, where driving through it leaves most of a
    // metre.
    assert!(
        overrun < RECONCILIATION_VISIBLE_CORRECTION_M,
        "the own kart was predicted {overrun} m past where the server put it"
    );
}
