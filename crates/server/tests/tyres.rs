//! Tyre life on the wire: the server's snapshots carry each kart's life, the
//! bots' falling as they drive with Tyres On and holding with Off.

use protocol::Snapshot;
use server::race::Race;
use sim::{BotLevel, Track, Tyres};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");

/// The first snapshot ten seconds into a race with one idle human and bots.
fn snapshot_ten_seconds_in(tyres: Tyres) -> Snapshot {
    let track = Track::from_sidecar(OVAL).expect("the oval loads");
    let mut race = Race::new(
        0x5EED,
        track,
        BotLevel::Basic,
        tyres,
        sim::RaceDistance::Sprint,
        vec![0],
        1,
    );
    loop {
        let ticked = race.tick();
        if race.ticks() >= 10 * sim::tuning::TICKS_PER_SECOND {
            if let Some(snapshot) = ticked.snapshot {
                return snapshot;
            }
        }
    }
}

#[test]
fn snapshots_carry_tyre_life_falling_with_tyres_on() {
    let snapshot = snapshot_ten_seconds_in(Tyres::On);
    assert_eq!(snapshot.karts[0].tyre_life, 100.0, "the idle human's stand");
    assert!(
        snapshot.karts[1..]
            .iter()
            .all(|kart| kart.tyre_life < 100.0),
        "every bot has driven on them"
    );
}

#[test]
fn snapshots_hold_tyre_life_at_new_with_tyres_off() {
    let snapshot = snapshot_ten_seconds_in(Tyres::Off);
    assert!(snapshot.karts.iter().all(|kart| kart.tyre_life == 100.0));
}
