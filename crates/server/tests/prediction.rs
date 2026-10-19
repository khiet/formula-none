//! Prediction of the own kart against the authoritative server (ADR 0009),
//! through the fake transport at the round trip and loss the ADR targets.
//! The client here is the browser's tick loop in Rust: it reconciles each
//! snapshot as it arrives and hands it to the interpolation of the other
//! karts, then predicts the tick with the input it sends against those
//! karts where it draws them. The scripted driver is the bot's driving, reading the kart as the client
//! draws it, so the run hops into the oval's drifts.

mod common;

use std::time::Duration;

use common::{race_together, read, snapshot_kart, wire};
use protocol::{ClientMessage, ServerMessage};
use server::fake_transport::{Conditions, Harness, TRACK};
use sim::bot::{self, Memory};
use sim::tuning::{
    COUNTDOWN_S, KART_HALF_LENGTH_M, RECONCILIATION_VISIBLE_CORRECTION_M, TICKS_PER_SECOND,
};
use sim::{Interpolation, KartInput, Prediction, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);

#[tokio::test(start_paused = true)]
async fn at_a_100_ms_round_trip_and_2_percent_loss_the_own_kart_is_corrected_out_of_sight() {
    let mut harness = Harness::new(Conditions {
        delay: Duration::from_millis(50),
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
    let mut ticks = tokio::time::interval(TICK);
    let mut sequence = 0;
    let mut corrections_m = Vec::new();
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
                );
                corrections_m.push(prediction.reconcile(
                    snapshot.tick,
                    snapshot.acknowledged[kart],
                    &snapshot_kart(&snapshot.karts[kart]),
                ));
            }
        }
        let input = bot::drive(
            prediction.track(),
            &[prediction.kart_transform()],
            &[prediction.standing()],
            0,
            &mut memory,
            TICK.as_secs_f32(),
        );
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
        prediction.corrections() > 0,
        "the loss corrected nothing, so it never reached the prediction"
    );
}

#[tokio::test(start_paused = true)]
async fn driving_into_another_kart_never_predicts_the_own_kart_visibly_past_where_the_server_puts_it(
) {
    let mut harness = Harness::new(Conditions {
        delay: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
        seed: 19,
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
    // Straight down the grid at full throttle, into the parked kart.
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
                );
                prediction.reconcile(
                    snapshot.tick,
                    acknowledged,
                    &snapshot_kart(&snapshot.karts[kart]),
                );
            }
        }
        sequence += 1;
        let input = wire(sequence, flat_out);
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
