//! Milestone M3 over the fake transport: eight humans race the oval with
//! all three items in play, each driving the bots' line and firing
//! whatever it rolls, and every one of them hears the same race: the same
//! events under the same ids, the same karts, hazards and projectiles in
//! every snapshot, and the same results.

mod common;

use std::collections::BTreeSet;
use std::time::Duration;

use common::{follow_the_line, race_together, seconds, wire, Heard};
use protocol::{EventKind, Item};
use server::fake_transport::{Conditions, Harness};
use sim::tuning::{COUNTDOWN_S, KARTS_PER_RACE, TICKS_PER_SECOND};
use sim::Track;

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);
/// Give up if the race has not ended by then: three laps of the oval take
/// a minute and a half at most, the cap allows a minute more, and the
/// outro follows.
const DEADLINE_S: f32 = 240.0;
/// Each kart sets off this long after the one before it, so the field is
/// spread out and every roll band is drawn from.
const STAGGER_S: f32 = 1.0;

#[tokio::test(start_paused = true)]
async fn eight_clients_race_with_all_three_items_and_hear_the_same_race() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players: Vec<_> = (0..KARTS_PER_RACE).map(|_| harness.connect()).collect();
    let countdowns = race_together(&code, &mut players).await;
    let grid: Vec<u8> = (0..KARTS_PER_RACE as u8).collect();
    assert!(countdowns.iter().all(|countdown| countdown.grid == grid));
    let track = Track::from_sidecar(OVAL).expect("the oval fixture loads");

    let mut heard: Vec<Heard> = (0..KARTS_PER_RACE).map(|_| Heard::default()).collect();
    let mut sequences = [0u32; KARTS_PER_RACE];
    let mut ticks = tokio::time::interval(TICK);
    loop {
        ticks.tick().await;
        for (player, heard) in players.iter_mut().zip(&mut heard) {
            heard.hear(player);
        }
        if heard.iter().all(|heard| heard.results.is_some()) {
            break;
        }
        let Some(newest) = heard[0].newest() else {
            continue;
        };
        let elapsed = seconds(newest.tick);
        assert!(
            elapsed < DEADLINE_S,
            "the race did not end in {DEADLINE_S} s"
        );
        for (kart, player) in players.iter().enumerate() {
            if elapsed < COUNTDOWN_S + kart as f32 * STAGGER_S {
                continue;
            }
            let own = &newest.karts[kart];
            let mut input = follow_the_line(&track, own);
            input.use_item = own.held_item.is_some();
            sequences[kart] += 1;
            player.send_input(wire(sequences[kart], input));
        }
    }

    // Every client heard the same events, in the same order under the
    // same ids, and the same results.
    let reference = &heard[0];
    for (kart, other) in heard.iter().enumerate().skip(1) {
        assert_eq!(other.events, reference.events, "client {kart}'s events");
        assert_eq!(other.results, reference.results, "client {kart}'s results");
    }
    let ids: BTreeSet<u32> = reference.events.iter().map(|event| event.id).collect();
    assert_eq!(ids.len(), reference.events.len(), "every id distinct");

    // All three items were fired, and something was hit.
    let fired = |wanted: Item| {
        reference
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::ItemFired { item, .. } if item == wanted))
    };
    for item in [Item::Boost, Item::Hazard, Item::Homing] {
        assert!(fired(item), "{item:?} was fired");
    }
    assert!(
        reference
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::Hit { .. })),
        "somebody was hit"
    );

    // Every snapshot two clients both received said the same of every
    // kart, hazard and projectile; and the snapshots showed a hazard on
    // the track and a projectile in flight at some point.
    for (kart, other) in heard.iter().enumerate().skip(1) {
        for (tick, snapshot) in &reference.snapshots {
            if let Some(theirs) = other.snapshots.get(tick) {
                assert_eq!(theirs.karts, snapshot.karts, "client {kart} at tick {tick}");
                assert_eq!(
                    theirs.hazards, snapshot.hazards,
                    "client {kart} at tick {tick}"
                );
                assert_eq!(
                    theirs.projectiles, snapshot.projectiles,
                    "client {kart} at tick {tick}"
                );
            }
        }
    }
    assert!(
        reference
            .snapshots
            .values()
            .any(|snapshot| !snapshot.hazards.is_empty()),
        "a hazard lay on the track"
    );
    assert!(
        reference
            .snapshots
            .values()
            .any(|snapshot| !snapshot.projectiles.is_empty()),
        "a projectile flew"
    );
    let results = reference.results.as_ref().unwrap();
    assert_eq!(results.len(), KARTS_PER_RACE);
    assert!(
        results
            .iter()
            .any(|standing| standing.finish_time_s.is_some()),
        "somebody finished"
    );
}
