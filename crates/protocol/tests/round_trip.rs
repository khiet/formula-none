//! Every message encodes to bytes and decodes back to itself through the
//! protocol's public seam (ADR 0004). The samples are the same values the
//! generated TypeScript is checked against in Vitest, which also holds them
//! to one per variant of the generated unions.

use protocol::{decode, encode, samples, ClientMessage, DecodeError, ServerMessage};

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
