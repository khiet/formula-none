//! Every message encodes to bytes and decodes back to itself through the
//! protocol's public seam (ADR 0004). The samples are the same values the
//! generated TypeScript is checked against in Vitest, so one list covers
//! both sides.

use protocol::{decode, encode, samples, ClientMessage, DecodeError, ServerMessage};

/// Every variant of each message enum, by the name serde gives it; a sample
/// must exist for each so no message type escapes the round trip.
const CLIENT_VARIANTS: [&str; 6] = ["ping", "join", "input", "ready", "start", "leave"];
const SERVER_VARIANTS: [&str; 7] = [
    "pong",
    "joined",
    "refused",
    "room",
    "countdown",
    "snapshot",
    "results",
];

#[test]
fn every_client_message_round_trips() {
    for (name, message) in samples::client() {
        let decoded: ClientMessage = decode(&encode(&message)).unwrap();
        assert_eq!(decoded, message, "{name}");
    }
}

#[test]
fn every_server_message_round_trips() {
    for (name, message) in samples::server() {
        let decoded: ServerMessage = decode(&encode(&message)).unwrap();
        assert_eq!(decoded, message, "{name}");
    }
}

#[test]
fn the_samples_cover_every_message_type_once() {
    let client: Vec<&str> = samples::client().iter().map(|(name, _)| *name).collect();
    let server: Vec<&str> = samples::server().iter().map(|(name, _)| *name).collect();
    assert_eq!(client, CLIENT_VARIANTS);
    assert_eq!(server, SERVER_VARIANTS);
}

#[test]
fn a_sample_is_named_after_its_serde_variant() {
    // The generator writes each sample under this name, and the Vitest side
    // matches the name against the generated union's keys.
    for (name, message) in samples::client() {
        assert_eq!(samples::variant_name(&message), name);
    }
    for (name, message) in samples::server() {
        assert_eq!(samples::variant_name(&message), name);
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
