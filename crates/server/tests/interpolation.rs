//! The other karts as the client draws them (ADR 0009): interpolated
//! between the snapshots it has received, 100 ms behind the newest, through
//! the fake transport. The client here is the browser's tick loop in Rust:
//! it hands each snapshot to the interpolation as it arrives, then advances
//! the interpolation one tick. The watching player sends nothing, so its
//! kart waits on the grid while the bots race.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use common::{projectiles, race_together, snapshot_kart};
use protocol::{EventKind, ServerMessage};
use server::fake_transport::{Client, Conditions, Harness};
use sim::tuning::{
    COUNTDOWN_S, INTERPOLATION_DELAY_S, INTERPOLATION_TELEPORT_M_PER_TICK, KARTS_PER_RACE,
    RECONCILIATION_VISIBLE_CORRECTION_M, SNAPSHOTS_PER_SECOND, TICKS_PER_SECOND,
};
use sim::Interpolation;

const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);
const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;
const DELAY_TICKS: f32 = INTERPOLATION_DELAY_S * TICKS_PER_SECOND as f32;
/// A respawn a snapshot carried, with the snapshot's tick.
struct Respawn {
    /// The event's id, which its repeats in the next snapshots share.
    id: u32,
    kart: usize,
    tick: u32,
}

/// One client tick: every snapshot that has arrived goes to the
/// interpolation, then it advances. `newest` keeps the newest snapshot tick
/// held and `held` every tick a snapshot arrived for. Returns each respawn
/// the snapshots carry, repeats included.
fn client_tick(
    player: &mut Client,
    interpolation: &mut Interpolation,
    newest: &mut u32,
    held: &mut BTreeSet<u32>,
) -> Vec<Respawn> {
    let mut respawned = Vec::new();
    while let Some(message) = player.try_recv() {
        if let ServerMessage::Snapshot(snapshot) = message {
            *newest = (*newest).max(snapshot.tick);
            held.insert(snapshot.tick);
            for event in &snapshot.events {
                if let EventKind::Respawned { kart } = event.kind {
                    respawned.push(Respawn {
                        id: event.id,
                        kart: usize::from(kart),
                        tick: snapshot.tick,
                    });
                }
            }
            interpolation.push(
                snapshot.tick,
                snapshot.karts.iter().map(snapshot_kart).collect(),
                projectiles(&snapshot),
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
        let code = harness.create_room();
        let mut players = [harness.connect()];
        race_together(&code, &mut players).await;
        let [mut player] = players;
        let mut interpolation = Interpolation::new();
        let mut ticks = tokio::time::interval(TICK);
        let mut newest = 0;
        let mut held = BTreeSet::new();
        let mut lags = Vec::new();
        // The newest snapshot itself lands up to the jitter early or late.
        let jitter_ticks = jitter_ms as f32 * TICKS_PER_SECOND as f32 / 1000.0;
        let swing = 1.0 + 2.0 * jitter_ticks;
        let end_tick = ((COUNTDOWN_S + 10.0) * TICKS_PER_SECOND as f32) as u32;
        while newest < end_tick {
            ticks.tick().await;
            client_tick(&mut player, &mut interpolation, &mut newest, &mut held);
            let Some(render) = interpolation.render_tick() else {
                continue;
            };
            assert!(
                render <= newest as f32,
                "at {jitter_ms} ms jitter the karts were drawn at tick {render}, past the newest snapshot {newest}"
            );
            // The first second settles the clock.
            if newest > TICKS_PER_SECOND {
                let lag = newest as f32 - render;
                assert!(
                    (lag - DELAY_TICKS).abs() <= swing,
                    "at {jitter_ms} ms jitter the karts were drawn {lag} ticks behind the newest snapshot"
                );
                lags.push(lag);
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
    // A seed whose race rolls no bot a rocket start, so the bots leave the
    // two watching karts on the grid untouched.
    let lossless = Conditions {
        delay: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
        seed: 2,
    };
    let mut harness = Harness::new(lossless);
    let code = harness.create_room();
    let mut players = [
        harness.connect(),
        harness.connect_under(Conditions {
            loss: 0.02,
            ..lossless
        }),
    ];
    race_together(&code, &mut players).await;
    let [mut reference, mut lossy] = players;

    let mut drawn = [Interpolation::new(), Interpolation::new()];
    let mut newest = [0, 0];
    let mut held = [BTreeSet::new(), BTreeSet::new()];
    let mut ticks = tokio::time::interval(TICK);
    let mut gaps = 0;
    let mut largest = (0.0, 0, 0);
    // A respawn is a jump on purpose, drawn on whichever tick each clock
    // reaches it; a kart is not compared around one. By event id.
    let mut respawns = BTreeMap::new();
    // Each drawing's karts a tick ago, and per kart the ticks on which it
    // moved further than any kart drives.
    let mut last_drawn: [Vec<[f32; 3]>; 2] = Default::default();
    let mut leaps = [vec![0; KARTS_PER_RACE], vec![0; KARTS_PER_RACE]];
    let end_tick = ((COUNTDOWN_S + 20.0) * TICKS_PER_SECOND as f32) as u32;
    while newest[0] < end_tick {
        ticks.tick().await;
        let before = newest[1];
        for respawn in client_tick(&mut reference, &mut drawn[0], &mut newest[0], &mut held[0]) {
            respawns.entry(respawn.id).or_insert(respawn);
        }
        client_tick(&mut lossy, &mut drawn[1], &mut newest[1], &mut held[1]);
        if before > 0 && newest[1] > before + TICKS_PER_SNAPSHOT {
            gaps += 1;
        }
        let karts = [drawn[0].kart_transforms(), drawn[1].kart_transforms()];
        for ((karts, last), leaps) in karts.iter().zip(&mut last_drawn).zip(&mut leaps) {
            for (kart, (now, last)) in karts.iter().zip(last.iter()).enumerate() {
                if distance(now.position, *last) > INTERPOLATION_TELEPORT_M_PER_TICK {
                    leaps[kart] += 1;
                }
            }
            *last = karts.iter().map(|kart| kart.position).collect();
        }
        let [expected, actual] = karts;
        let render = drawn[0].render_tick().unwrap_or(0.0);
        // Across a lost snapshot the lossy client has nothing to draw from
        // but the snapshots either side, and behind the newest its clock
        // lags: those ticks are the loss, not the interpolation.
        let lossy_render = drawn[1].render_tick().unwrap_or(0.0) as u32;
        let interval_start = lossy_render / TICKS_PER_SNAPSHOT * TICKS_PER_SNAPSHOT;
        let across_a_gap = !held[1].contains(&interval_start)
            || !held[1].contains(&(interval_start + TICKS_PER_SNAPSHOT));
        if newest[1] < newest[0] || across_a_gap {
            continue;
        }
        let respawning = |kart: usize| {
            respawns.values().any(|respawn: &Respawn| {
                respawn.kart == kart
                    && (render - respawn.tick as f32).abs() <= 2.0 * TICKS_PER_SNAPSHOT as f32
            })
        };
        for (kart, (expected, actual)) in expected.iter().zip(&actual).enumerate() {
            if respawning(kart) {
                continue;
            }
            let off = distance(actual.position, expected.position);
            if off > largest.0 {
                largest = (off, kart, newest[0]);
            }
        }
    }

    assert!(gaps > 5, "only {gaps} gaps in the snapshots");
    // Put back on the track, a kart is drawn there in one move, never
    // streaking across the track between.
    assert!(!respawns.is_empty(), "no kart respawned");
    let mut respawned = vec![0; KARTS_PER_RACE];
    for respawn in respawns.values() {
        respawned[respawn.kart] += 1;
    }
    let [without_loss, with_loss] = leaps;
    assert_eq!(
        without_loss, respawned,
        "leaps per kart against respawns, without loss"
    );
    assert_eq!(
        with_loss, respawned,
        "leaps per kart against respawns, with loss"
    );
    let (off, kart, tick) = largest;
    assert!(
        off < RECONCILIATION_VISIBLE_CORRECTION_M,
        "kart {kart} was drawn {off} m from where the snapshots put it, by tick {tick}"
    );
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let [x, y, z] = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (x * x + y * y + z * z).sqrt()
}
