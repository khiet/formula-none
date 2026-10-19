//! Prediction of the own kart against the authoritative server (ADR 0009),
//! through the fake transport at the round trip and loss the ADR targets.
//! The client here is the browser's tick loop in Rust: it reconciles each
//! snapshot as it arrives, then predicts the tick with the input it sends.
//! The scripted driver is the bot's driving, reading the kart as the client
//! draws it, so the run hops into the oval's drifts.
//!
//! The grid's bots race the same track, and kart-to-kart contact is the
//! server's alone (ADR 0009): until the other karts are local bodies, a
//! snapshot after a touch corrects what no network could have caused, so
//! the measure leaves those out.

use std::time::Duration;

use protocol::{ClientMessage, DriftDirection, Input, KartSnapshot, ServerMessage};
use server::fake_transport::{Conditions, Harness, TRACK};
use sim::bot::{self, Memory};
use sim::tuning::{
    COUNTDOWN_S, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M, RECONCILIATION_VISIBLE_CORRECTION_M,
    TICKS_PER_SECOND,
};
use sim::{Drift, KartInput, Prediction, SnapshotKart, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);
/// Centre to centre, past any reach at which two karts' boxes can touch.
const CONTACT_REACH_M: f32 = 2.0 * (KART_HALF_LENGTH_M + KART_HALF_WIDTH_M);
/// Ticks after a snapshot that showed a kart within reach for which a
/// correction may still be the touch's: the round trip and a snapshot more.
const AFTER_CONTACT_TICKS: u32 = TICKS_PER_SECOND / 4;

/// The wire input for the sim's: each control quantised to its byte, as the
/// browser's room client sends it.
fn wire(sequence: u32, input: KartInput) -> Input {
    let unit = |value: f32| (value.clamp(0.0, 1.0) * f32::from(u8::MAX)).round() as u8;
    Input {
        sequence,
        steer: (input.steer.clamp(-1.0, 1.0) * f32::from(i8::MAX)).round() as i8,
        throttle: unit(input.throttle),
        brake: unit(input.brake),
        hop: input.hop,
        use_item: input.use_item,
        hold_item: input.hold_item,
    }
}

/// The sim's input for a wire input, as the server reads it.
fn read(input: Input) -> KartInput {
    KartInput {
        steer: input.steer_unit(),
        throttle: input.throttle_unit(),
        brake: input.brake_unit(),
        hop: input.hop,
        use_item: input.use_item,
        hold_item: input.hold_item,
    }
}

/// Whether another kart is within reach of `kart` in the snapshot, on the
/// ground plane.
fn within_reach(karts: &[KartSnapshot], kart: usize) -> bool {
    let [x, _, z] = karts[kart].position;
    karts.iter().enumerate().any(|(other, snapshot)| {
        let [ox, _, oz] = snapshot.position;
        other != kart && (ox - x).hypot(oz - z) < CONTACT_REACH_M
    })
}

fn snapshot_kart(kart: &KartSnapshot) -> SnapshotKart {
    SnapshotKart {
        position: kart.position,
        rotation: kart.rotation,
        velocity: kart.velocity,
        drift: kart.drift.map(|drift| Drift {
            direction: match drift.direction {
                DriftDirection::Left => sim::DriftDirection::Left,
                DriftDirection::Right => sim::DriftDirection::Right,
            },
            charge_s: drift.charge_s,
        }),
        lap: u32::from(kart.lap),
        checkpoint: usize::from(kart.checkpoint),
        spline_progress: kart.spline_progress,
    }
}

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
    let mut last_contact_tick = None;
    // Twenty seconds of racing: both of the oval's turns, each lap.
    let end_tick = ((COUNTDOWN_S + 20.0) * TICKS_PER_SECOND as f32) as u32;
    while newest_tick < end_tick {
        ticks.tick().await;
        while let Some(message) = player.try_recv() {
            if let ServerMessage::Snapshot(snapshot) = message {
                newest_tick = newest_tick.max(snapshot.tick);
                if within_reach(&snapshot.karts, kart) {
                    last_contact_tick = Some(snapshot.tick);
                }
                let corrected_m = prediction.reconcile(
                    snapshot.tick,
                    snapshot.acknowledged[kart],
                    &snapshot_kart(&snapshot.karts[kart]),
                );
                let after_contact = last_contact_tick
                    .is_some_and(|contact| snapshot.tick <= contact + AFTER_CONTACT_TICKS);
                if !after_contact {
                    corrections_m.push(corrected_m);
                }
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
        prediction.tick(sequence, read(input));
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
