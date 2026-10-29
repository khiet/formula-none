//! Which lane a message rides (ADR 0011): inputs and snapshots go on
//! datagrams, where a lost or late one is simply superseded by the next;
//! everything else goes on the reliable stream. A transport without
//! datagrams, the WebSocket fallback, carries both lanes on its one
//! channel. The session and the rooms choose the lane; a transport only
//! honours it.

use protocol::{encode, ClientMessage, ServerMessage};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Delivered in order and never lost.
    Reliable,
    /// May be lost or reordered; for what the next message supersedes.
    Datagram,
}

/// Bytes on their way to one client with the lane they ride.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub lane: Lane,
    pub bytes: Vec<u8>,
}

/// Bytes on their way to one client, whichever transport carries them.
pub type Outbound = mpsc::UnboundedSender<Outgoing>;

impl Outgoing {
    /// `message` encoded for the lane its kind rides.
    pub fn from_server(message: &ServerMessage) -> Self {
        Self {
            lane: server_lane(message),
            bytes: encode(message),
        }
    }

    /// `message` encoded for the lane its kind rides.
    pub fn from_client(message: &ClientMessage) -> Self {
        Self {
            lane: client_lane(message),
            bytes: encode(message),
        }
    }
}

/// A Pong answers the Ping datagram that proves a transport, so it rides
/// a datagram too; the server's own Ping probes the lane the snapshots
/// ride (ADR 0018).
pub fn server_lane(message: &ServerMessage) -> Lane {
    match message {
        ServerMessage::Snapshot(_) | ServerMessage::Pong { .. } | ServerMessage::Ping { .. } => {
            Lane::Datagram
        }
        ServerMessage::Joined { .. }
        | ServerMessage::Refused(_)
        | ServerMessage::Room(_)
        | ServerMessage::Countdown { .. }
        | ServerMessage::Events(_)
        | ServerMessage::Results { .. }
        | ServerMessage::Replaced => Lane::Reliable,
    }
}

pub fn client_lane(message: &ClientMessage) -> Lane {
    match message {
        ClientMessage::Input(_) | ClientMessage::Ping { .. } | ClientMessage::Pong { .. } => {
            Lane::Datagram
        }
        ClientMessage::Join { .. }
        | ClientMessage::Ready { .. }
        | ClientMessage::Start
        | ClientMessage::Rematch
        | ClientMessage::Leave
        | ClientMessage::Rejoin { .. }
        | ClientMessage::RaceReport { .. } => Lane::Reliable,
    }
}
