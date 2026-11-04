//! Coins on the server's race (ADR 0055): the server holds every kart's
//! count, a pickup is a server event naming the coin, and the next
//! snapshot carries the count and the coin gone. Driven at the race's own
//! seam, `Race::tick`, one human idle on a coin stacked under its slot.

use protocol::{EventKind, Snapshot};
use server::race::Race;
use sim::{BotLevel, Track, Tyres};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");

/// The oval with its own coins and one more under spawn slot 0, which is
/// the last in the sidecar's order.
fn oval_with_a_coin_under_slot_0() -> (Track, u8) {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    let under = sidecar["spawn_grid"][0]["position"].clone();
    let coins = sidecar["coins"]
        .as_array_mut()
        .expect("the oval lists its coins");
    coins.push(serde_json::json!({ "position": under, "yaw": 0.0 }));
    let coin = (coins.len() - 1) as u8;
    let track = Track::from_sidecar(&sidecar.to_string()).expect("the edited oval loads");
    (track, coin)
}

#[test]
fn a_pickup_is_a_server_event_and_the_next_snapshot_carries_the_count_and_the_coin_gone() {
    let (track, coin) = oval_with_a_coin_under_slot_0();
    let mut race = Race::new(0x5EED, track, BotLevel::Basic, Tyres::Off, 3, vec![0], 1);

    let mut picked = None;
    let mut after: Option<Snapshot> = None;
    while after.is_none() {
        let ticked = race.tick();
        assert!(
            race.ticks() < 10 * sim::tuning::TICKS_PER_SECOND,
            "no pickup and snapshot by ten seconds"
        );
        if let Some(event) = ticked
            .events
            .iter()
            .find(|event| matches!(event.kind, EventKind::CoinPicked { kart: 0, .. }))
        {
            picked = Some(event.kind);
        }
        if let Some(snapshot) = ticked.snapshot {
            match picked {
                None => assert_eq!(snapshot.karts[0].coins, 0, "none before the pickup"),
                Some(_) => after = Some(snapshot),
            }
        }
    }

    assert_eq!(picked, Some(EventKind::CoinPicked { kart: 0, coin }));
    let snapshot = after.expect("a snapshot after the pickup");
    assert_eq!(snapshot.karts[0].coins, 1);
    assert!(
        !snapshot.coins_present[usize::from(coin)],
        "the coin is gone"
    );
    assert!(
        snapshot.coins_present[..usize::from(coin)]
            .iter()
            .all(|&present| present),
        "the oval's own coins are there to pick up"
    );
}
