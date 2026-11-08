//! Tyre life on the wire: the server's snapshots carry each kart's life, the
//! bots' falling as they drive with Tyres On and holding with Off.

use protocol::Snapshot;
use server::race::Race;
use sim::{rain_lap, Picks, Track, Tyres, Weather};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");

/// The first snapshot ten seconds into a race with one idle human and bots.
fn snapshot_ten_seconds_in(tyres: Tyres) -> Snapshot {
    let track = Track::from_sidecar(OVAL).expect("the oval loads");
    let mut race = Race::new(
        0x5EED,
        track,
        Picks {
            tyres,
            ..Picks::default()
        },
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

#[test]
fn snapshots_carry_the_tick_the_rain_began_once_it_has() {
    let track = Track::from_sidecar(OVAL).expect("the oval loads");
    let picks = Picks {
        weather: Weather::Rain,
        ..Picks::default()
    };
    // A seed whose rain comes on lap 2, which the bots reach first.
    let seed = (0..).find(|&seed| rain_lap(seed, picks.laps) == 2).unwrap();
    let mut race = Race::new(seed, track, picks, vec![0], 1);
    let began = loop {
        assert!(race.ticks() < 60 * sim::tuning::TICKS_PER_SECOND, "no rain");
        let Some(snapshot) = race.tick().snapshot else {
            continue;
        };
        let lap_two = snapshot.karts.iter().any(|kart| kart.lap >= 2);
        if let Some(began) = snapshot.rain_began_tick {
            assert!(lap_two, "it began with a kart on the lap");
            assert!(began <= snapshot.tick);
            break began;
        }
        assert!(!lap_two, "a kart is on the lap and the snapshot says dry");
    };
    let later = loop {
        if let Some(snapshot) = race.tick().snapshot {
            break snapshot;
        }
    };
    assert_eq!(later.rain_began_tick, Some(began), "the word never changes");
}

#[test]
fn a_dry_race_never_sends_rain() {
    assert_eq!(snapshot_ten_seconds_in(Tyres::Off).rain_began_tick, None);
}
