//! One client's connection, whichever transport carries it (ADR 0011): the
//! bytes it sends come in on a channel and the bytes it is sent go out on
//! another, so the WebSocket, WebTransport and the fake transport all drive
//! the same code. A Ping is answered at any time; the first Join seats the
//! client in a room, or refuses it and ends the session.

use protocol::{decode, encode, ClientMessage, ServerMessage};
use tokio::sync::mpsc;

use crate::rooms::{Membership, Outbound, Rooms};
use crate::Shutdown;

/// The client's side of a session: bytes in, bytes out. A transport pumps
/// its socket into `inbound` and drains `outbound` into the socket.
pub struct Channels {
    pub inbound: mpsc::UnboundedReceiver<Vec<u8>>,
    pub outbound: Outbound,
}

/// Serve one client until it disconnects, is refused, or the server stops.
/// Returning drops the client's membership, which tells the room.
pub async fn run(rooms: Rooms, channels: Channels, shutdown: Shutdown) {
    let Channels {
        mut inbound,
        outbound,
    } = channels;
    let mut membership: Option<Membership> = None;
    loop {
        let bytes = tokio::select! {
            () = shutdown.wait() => return,
            received = inbound.recv() => match received {
                Some(bytes) => bytes,
                None => return,
            },
        };
        let Ok(message) = decode::<ClientMessage>(&bytes) else {
            // Not a message: the client carries on, as before the rooms.
            continue;
        };
        match (message, &membership) {
            (ClientMessage::Ping { nonce }, _) => {
                if outbound
                    .send(encode(&ServerMessage::Pong { nonce }))
                    .is_err()
                {
                    return;
                }
            }
            (
                ClientMessage::Join {
                    version,
                    room,
                    nickname,
                },
                None,
            ) => match rooms.join(&room, version, nickname, outbound.clone()).await {
                Ok(joined) => membership = Some(joined),
                Err(refusal) => {
                    let _ = outbound.send(encode(&ServerMessage::Refused(refusal)));
                    return;
                }
            },
            (ClientMessage::Join { .. }, Some(_)) => {}
            // A Leave ends the session: a Join is the first message on a
            // connection, so the client connects again for another room.
            (ClientMessage::Leave, Some(_)) => {
                if let Some(joined) = membership.take() {
                    joined.leave();
                }
                return;
            }
            (message, Some(joined)) => joined.send(message),
            // Room messages before a Join have no room to go to.
            (_, None) => {}
        }
    }
}
