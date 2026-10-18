//! Every message encodes to bytes and decodes back to itself through the
//! protocol's public seam (ADR 0004).

use protocol::{decode, encode, ClientMessage, DecodeError, ServerMessage};

#[test]
fn ping_round_trips() {
    let message = ClientMessage::Ping { nonce: 0xdead_beef };
    let decoded: ClientMessage = decode(&encode(&message)).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn pong_round_trips() {
    let message = ServerMessage::Pong { nonce: 7 };
    let decoded: ServerMessage = decode(&encode(&message)).unwrap();
    assert_eq!(decoded, message);
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
