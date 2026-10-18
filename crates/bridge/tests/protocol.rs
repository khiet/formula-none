//! The bridge's protocol surface: the browser encodes and decodes messages
//! through it so the wire format exists only in Rust (ADR 0004).

use bridge::{decode_pong, encode_ping};
use protocol::{decode, encode, ClientMessage, ServerMessage};

#[test]
fn encode_ping_is_the_protocol_encoding() {
    assert_eq!(encode_ping(42), encode(&ClientMessage::Ping { nonce: 42 }));
    let decoded: ClientMessage = decode(&encode_ping(7)).unwrap();
    assert_eq!(decoded, ClientMessage::Ping { nonce: 7 });
}

#[test]
fn decode_pong_reads_the_server_encoding() {
    let bytes = encode(&ServerMessage::Pong { nonce: 42 });
    assert_eq!(decode_pong(&bytes), Ok(42));
}

#[test]
fn decode_pong_reports_bytes_that_are_not_a_pong() {
    let result = decode_pong(&[0xff, 0xff]);
    assert!(result.is_err(), "{result:?}");
}
