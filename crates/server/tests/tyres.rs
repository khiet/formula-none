//! Tyres on the server's race (ADR 0055): the server holds every kart's
//! count, a pickup is a server event naming the tyre, and the next
//! snapshot carries the count and the tyre gone. Driven at the race's own
//! seam, `Race::tick`, one human idle on a tyre stacked under its slot.

use protocol::{EventKind, Snapshot};
use server::race::Race;
use sim::Track;

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");

/// The oval with its own tyres and one more under spawn slot 0, which is
/// the last in the sidecar's order.
fn oval_with_a_tyre_under_slot_0() -> (Track, u8) {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    let under = sidecar["spawn_grid"][0]["position"].clone();
    let tyres = sidecar["tyres"]
        .as_array_mut()
        .expect("the oval lists its tyres");
    tyres.push(serde_json::json!({ "position": under, "yaw": 0.0 }));
    let tyre = (tyres.len() - 1) as u8;
    let track = Track::from_sidecar(&sidecar.to_string()).expect("the edited oval loads");
    (track, tyre)
}

#[test]
fn a_pickup_is_a_server_event_and_the_next_snapshot_carries_the_count_and_the_tyre_gone() {
    let (track, tyre) = oval_with_a_tyre_under_slot_0();
    let mut race = Race::new(0x5EED, track, vec![0], 1);

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
            .find(|event| matches!(event.kind, EventKind::TyrePicked { kart: 0, .. }))
        {
            picked = Some(event.kind);
        }
        if let Some(snapshot) = ticked.snapshot {
            match picked {
                None => assert_eq!(snapshot.karts[0].tyres, 0, "none before the pickup"),
                Some(_) => after = Some(snapshot),
            }
        }
    }

    assert_eq!(picked, Some(EventKind::TyrePicked { kart: 0, tyre }));
    let snapshot = after.expect("a snapshot after the pickup");
    assert_eq!(snapshot.karts[0].tyres, 1);
    assert!(
        !snapshot.tyres_present[usize::from(tyre)],
        "the tyre is gone"
    );
    assert!(
        snapshot.tyres_present[..usize::from(tyre)]
            .iter()
            .all(|&present| present),
        "the oval's own tyres are there to pick up"
    );
}
