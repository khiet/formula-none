//! Every message encodes to bytes and decodes back to itself through the
//! protocol's public seam (ADR 0004). The samples are the same values the
//! generated TypeScript is checked against in Vitest, which also holds them
//! to one per variant of the generated unions.

use protocol::{
    decode, encode, samples, ClientMessage, DecodeError, Room, ServerMessage, Snapshot,
};
use sim::tuning::LAP_CHOICES;
use sim::{BotLevel, Tyres};

#[test]
fn every_client_message_round_trips() {
    for message in samples::client() {
        let decoded: ClientMessage = decode(&encode(&message)).unwrap();
        assert_eq!(decoded, message);
    }
}

#[test]
fn every_server_message_round_trips() {
    for message in samples::server() {
        let decoded: ServerMessage = decode(&encode(&message)).unwrap();
        assert_eq!(decoded, message);
    }
}

#[test]
fn a_ping_is_compact() {
    // Postcard: one byte of variant, then a varint nonce.
    assert_eq!(encode(&ClientMessage::Ping { nonce: 1 }), [0, 1]);
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = encode(&ClientMessage::Ping { nonce: 1 });
    bytes.push(0);
    let result: Result<ClientMessage, DecodeError> = decode(&bytes);
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn garbage_is_rejected() {
    let result: Result<ServerMessage, DecodeError> = decode(&[0xff, 0xff]);
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn a_room_and_a_countdown_round_trip_at_every_lap_choice() {
    for laps in LAP_CHOICES {
        let room = ServerMessage::Room(Room {
            laps,
            ..samples::room()
        });
        let decoded: ServerMessage = decode(&encode(&room)).unwrap();
        assert_eq!(decoded, room);

        let countdown = ServerMessage::Countdown {
            seed: 7,
            track: "oval".to_owned(),
            bot_level: BotLevel::Basic,
            tyres: Tyres::Off,
            laps,
            grid: vec![0],
        };
        let decoded: ServerMessage = decode(&encode(&countdown)).unwrap();
        assert_eq!(decoded, countdown);
    }
}

#[test]
fn a_room_and_a_countdown_round_trip_at_every_tyres_pick() {
    for tyres in Tyres::ALL {
        let room = ServerMessage::Room(Room {
            tyres,
            ..samples::room()
        });
        let decoded: ServerMessage = decode(&encode(&room)).unwrap();
        assert_eq!(decoded, room);

        let countdown = ServerMessage::Countdown {
            seed: 7,
            track: "oval".to_owned(),
            bot_level: BotLevel::Basic,
            tyres,
            laps: 3,
            grid: vec![0],
        };
        let decoded: ServerMessage = decode(&encode(&countdown)).unwrap();
        assert_eq!(decoded, countdown);
    }
}

#[test]
fn a_room_and_a_countdown_round_trip_at_every_bot_level() {
    for level in BotLevel::ALL {
        let room = ServerMessage::Room(Room {
            bot_level: level,
            ..samples::room()
        });
        let decoded: ServerMessage = decode(&encode(&room)).unwrap();
        assert_eq!(decoded, room);

        let countdown = ServerMessage::Countdown {
            seed: 7,
            track: "oval".to_owned(),
            bot_level: level,
            tyres: Tyres::Off,
            laps: 3,
            grid: vec![0],
        };
        let decoded: ServerMessage = decode(&encode(&countdown)).unwrap();
        assert_eq!(decoded, countdown);
    }
}

#[test]
fn a_kart_on_the_pit_lane_is_still_on_it_after_the_trip() {
    let mut snapshot = samples::snapshot();
    for (slot, kart) in snapshot.karts.iter_mut().enumerate() {
        kart.in_pit_lane = slot % 2 == 0;
    }
    let message = ServerMessage::Snapshot(snapshot);
    let ServerMessage::Snapshot(Snapshot { karts, .. }) = decode(&encode(&message)).unwrap() else {
        panic!("a snapshot decodes as a snapshot");
    };
    let in_lane: Vec<bool> = karts.iter().map(|kart| kart.in_pit_lane).collect();
    assert_eq!(
        in_lane,
        [true, false, true, false, true, false, true, false]
    );
}
