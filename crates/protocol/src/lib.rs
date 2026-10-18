//! Wire messages between client and server, defined once here. The web app's
//! TypeScript types are generated from this crate, never written by hand.
//!
//! Every message is serialised with postcard (ADR 0004). The encoding is
//! the same whichever transport carries it (ADR 0011).

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;

/// What a client sends the server.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    /// Ask for a [`ServerMessage::Pong`] carrying the same nonce; the M0(b)
    /// round trip that proves a transport.
    Ping { nonce: u32 },
}

/// What the server sends a client.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ServerMessage {
    /// The answer to a [`ClientMessage::Ping`] with that ping's nonce.
    Pong { nonce: u32 },
}

/// The bytes a message travels as.
pub fn encode<M: Serialize>(message: &M) -> Vec<u8> {
    postcard::to_allocvec(message).expect("messages hold nothing postcard cannot encode")
}

/// The message `bytes` carry. Fails when they are not exactly one message.
pub fn decode<M: DeserializeOwned>(bytes: &[u8]) -> Result<M, DecodeError> {
    let (message, rest) = postcard::take_from_bytes(bytes).map_err(DecodeError::Malformed)?;
    if rest.is_empty() {
        Ok(message)
    } else {
        Err(DecodeError::TrailingBytes(rest.len()))
    }
}

#[derive(Debug)]
pub enum DecodeError {
    Malformed(postcard::Error),
    /// Bytes followed a complete message.
    TrailingBytes(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => write!(f, "malformed message: {error}"),
            Self::TrailingBytes(count) => write!(f, "{count} bytes after the message"),
        }
    }
}

impl std::error::Error for DecodeError {}
