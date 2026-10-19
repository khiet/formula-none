//! The other karts as the client draws them (ADR 0009): interpolated
//! between the snapshots it has received, 100 ms behind the newest, through
//! the fake transport. The client here is the browser's tick loop in Rust:
//! it hands each snapshot to the interpolation as it arrives, then advances
//! the interpolation one tick. The watching player sends nothing, so its
//! kart waits on the grid while the bots race.

mod common;

use std::time::Duration;

use common::snapshot_kart;
use protocol::{ClientMessage, EventKind, PlayerState, ServerMessage};
use server::fake_transport::{Client, Conditions, Harness};
use sim::tuning::{
    COUNTDOWN_S, INTERPOLATION_DELAY_S, RECONCILIATION_VISIBLE_CORRECTION_M, SNAPSHOTS_PER_SECOND,
    TICKS_PER_SECOND,
};
use sim::Interpolation;

const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);
const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;
const DELAY_TICKS: f32 = INTERPOLATION_DELAY_S * TICKS_PER_SECOND as f32;

/// A player alone in a room that has started its race, and its kart.
async fn racing_alone(harness: &mut Harness) -> (Client, usize) {
    let code = harness.create_room();
    let mut player = harness.connect();
    let (slot, _) = player.join(&code, "watcher").await.unwrap();
    player.send(&ClientMessage::Ready { ready: true });
    player.next_room().await.unwrap();
    player.send(&ClientMessage::Start);
    let countdown = player.next_countdown().await.unwrap();
    let kart = countdown
        .grid
        .iter()
        .position(|&seat| seat == slot)
        .expect("the player is on the grid");
    (player, kart)
}

/// One client tick: every snapshot that has arrived goes to the
/// interpolation, then it advances. `newest` keeps the newest snapshot tick
/// held. Returns each kart the snapshots say respawned, with the tick of
/// the snapshot that said so.
fn client_tick(
    player: &mut Client,
    interpolation: &mut Interpolation,
    newest: &mut u32,
) -> Vec<(usize, u32)> {
    let mut respawned = Vec::new();
    while let Some(message) = player.try_recv() {
        if let ServerMessage::Snapshot(snapshot) = message {
            *newest = (*newest).max(snapshot.tick);
            for event in &snapshot.events {
                if let EventKind::Respawned { kart } = event.kind {
                    respawned.push((usize::from(kart), snapshot.tick));
                }
            }
            interpolation.push(
                snapshot.tick,
                snapshot.karts.iter().map(snapshot_kart).collect(),
            );
        }
    }
    interpolation.advance();
    respawned
}

#[tokio::test(start_paused = true)]
async fn at_any_jitter_the_other_karts_are_drawn_100_ms_behind_the_newest_snapshot_and_never_ahead_of_it(
) {
    for jitter_ms in [0, 10, 25, 50] {
        let mut harness = Harness::new(Conditions {
            delay: Duration::from_millis(50),
            jitter: Duration::from_millis(jitter_ms),
            loss: 0.0,
            seed: 19,
        });
        let (mut player, _) = racing_alone(&mut harness).await;
        let mut interpolation = Interpolation::new();
        let mut ticks = tokio::time::interval(TICK);
        let mut newest = 0;
        let mut lags = Vec::new();
        let end_tick = ((COUNTDOWN_S + 10.0) * TICKS_PER_SECOND as f32) as u32;
        while newest < end_tick {
            ticks.tick().await;
            client_tick(&mut player, &mut interpolation, &mut newest);
            let Some(render) = interpolation.render_tick() else {
                continue;
            };
            assert!(
                render <= newest as f32,
                "at {jitter_ms} ms jitter the karts were drawn at tick {render}, past the newest snapshot {newest}"
            );
            // The first second settles the clock.
            if newest > TICKS_PER_SECOND {
                lags.push(newest as f32 - render);
            }
        }
        let mean = lags.iter().sum::<f32>() / lags.len() as f32;
        assert!(
            (mean - DELAY_TICKS).abs() <= 1.0,
            "at {jitter_ms} ms jitter the karts were drawn {mean} ticks behind the newest snapshot on average"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn at_a_100_ms_round_trip_and_2_percent_loss_the_other_karts_are_drawn_where_they_would_be_without_it(
) {
    let lossless = Conditions {
        delay: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
        seed: 19,
    };
    let mut harness = Harness::new(lossless);
    let code = harness.create_room();
    let mut reference = harness.connect();
    let mut lossy = harness.connect_under(Conditions {
        loss: 0.02,
        ..lossless
    });
    reference.join(&code, "reference").await.unwrap();
    lossy.join(&code, "lossy").await.unwrap();
    for player in [&reference, &lossy] {
        player.send(&ClientMessage::Ready { ready: true });
    }
    while !reference
        .next_room()
        .await
        .unwrap()
        .players
        .iter()
        .all(|player| player.state == PlayerState::Ready)
    {}
    reference.send(&ClientMessage::Start);
    reference.next_countdown().await.unwrap();
    lossy.next_countdown().await.unwrap();

    let mut drawn = [Interpolation::new(), Interpolation::new()];
    let mut newest = [0, 0];
    let mut ticks = tokio::time::interval(TICK);
    let mut gaps = 0;
    let mut largest = (0.0, 0, 0);
    // A respawn is a jump on purpose, drawn on whichever tick each clock
    // reaches it; a kart is not compared around one.
    let mut respawns = Vec::new();
    let end_tick = ((COUNTDOWN_S + 20.0) * TICKS_PER_SECOND as f32) as u32;
    while newest[0] < end_tick {
        ticks.tick().await;
        let before = newest[1];
        respawns.extend(client_tick(&mut reference, &mut drawn[0], &mut newest[0]));
        client_tick(&mut lossy, &mut drawn[1], &mut newest[1]);
        if before > 0 && newest[1] > before + TICKS_PER_SNAPSHOT {
            gaps += 1;
        }
        let [expected, actual] = [drawn[0].kart_transforms(), drawn[1].kart_transforms()];
        let render = drawn[0].render_tick().unwrap_or(0.0);
        let respawning = |kart: usize| {
            respawns.iter().any(|&(respawned, tick)| {
                respawned == kart && (render - tick as f32).abs() <= 2.0 * TICKS_PER_SNAPSHOT as f32
            })
        };
        for (kart, (expected, actual)) in expected.iter().zip(&actual).enumerate() {
            if respawning(kart) {
                continue;
            }
            let [x, y, z] = difference(actual.position, expected.position);
            let off = (x * x + y * y + z * z).sqrt();
            if off > largest.0 {
                largest = (off, kart, newest[0]);
            }
        }
    }

    assert!(gaps > 5, "only {gaps} gaps in the snapshots");
    let (off, kart, tick) = largest;
    assert!(
        off < RECONCILIATION_VISIBLE_CORRECTION_M,
        "kart {kart} was drawn {off} m from where the snapshots put it, by tick {tick}"
    );
}

fn difference(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
