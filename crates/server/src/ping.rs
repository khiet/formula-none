//! The M0(b) round trip: a Ping comes back as a Pong with the same nonce.
//! Both transports hand their bytes here, so the encoding is shared by
//! construction (ADR 0011).

use protocol::{decode, encode, ClientMessage, ServerMessage};

/// The bytes to send back for `bytes` from a client, or `None` when they
/// are not a message the server answers yet; a session ignores those and
/// carries on. The room messages get their handlers with the rooms.
pub fn reply(bytes: &[u8]) -> Option<Vec<u8>> {
    match decode::<ClientMessage>(bytes) {
        Ok(ClientMessage::Ping { nonce }) => Some(encode(&ServerMessage::Pong { nonce })),
        _ => None,
    }
}
