//! An in-process stand-in for the network: a server with its rooms and up
//! to a grid of clients, joined by channels that add delay, jitter and loss
//! on the way. The room tests run on it, and the netcode tickets extend it
//! (ADR 0009, ADR 0012). Run under tokio's paused clock, a five-minute
//! wait takes no time at all.

use std::time::Duration;

use protocol::{decode, encode, ClientMessage, Refusal, Room, ServerMessage, PROTOCOL_VERSION};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::sync::{mpsc, watch};

use crate::rooms::Rooms;
use crate::session::{self, Channels};
use crate::Shutdown;

/// What the fake network does to every message, in each direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    /// Base one-way delay.
    pub delay: Duration,
    /// Each message's delay varies by up to this much either way. Messages
    /// keep their order, as on a reliable channel; the unreliable lane that
    /// reorders is the netcode tickets' to add.
    pub jitter: Duration,
    /// Probability in 0..=1 that a message never arrives.
    pub loss: f64,
    /// Seeds the loss and jitter draws, so a run repeats.
    pub seed: u64,
}

impl Conditions {
    /// No delay, no jitter, no loss.
    pub const PERFECT: Self = Self {
        delay: Duration::ZERO,
        jitter: Duration::ZERO,
        loss: 0.0,
        seed: 0,
    };
}

/// How long a client waits for a message before giving up, in the fake
/// clock; generous because paused time passes only while every task waits.
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(10);

/// A server and the network to it. Dropping it stops the server.
pub struct Harness {
    rooms: Rooms,
    conditions: Conditions,
    shutdown: Shutdown,
    stop: watch::Sender<bool>,
    next_seed: u64,
}

impl Harness {
    pub fn new(conditions: Conditions) -> Self {
        let (stop, shutdown) = Shutdown::pair();
        Self {
            rooms: Rooms::new(shutdown.clone()),
            conditions,
            shutdown,
            stop,
            next_seed: conditions.seed,
        }
    }

    /// The server's rooms, as the HTTP endpoint and the sessions see them.
    pub fn rooms(&self) -> &Rooms {
        &self.rooms
    }

    /// What `POST /api/rooms` does: a new room's code.
    pub fn create_room(&self) -> String {
        self.rooms.create()
    }

    /// A client connected to the server through the fake network, not yet
    /// in any room.
    pub fn connect(&mut self) -> Client {
        let (client_to_server, server_inbound) = pump(self.conditions, self.next_seed());
        let (server_outbound, client_from_server) = pump(self.conditions, self.next_seed());
        tokio::spawn(session::run(
            self.rooms.clone(),
            Channels {
                inbound: server_inbound,
                outbound: server_outbound,
            },
            self.shutdown.clone(),
        ));
        Client {
            to_server: client_to_server,
            from_server: client_from_server,
        }
    }

    /// Stop the server: every session and room task ends.
    pub fn stop(&self) {
        let _ = self.stop.send(true);
    }

    fn next_seed(&mut self) -> u64 {
        self.next_seed = self.next_seed.wrapping_add(1);
        self.next_seed
    }
}

/// One client's end of the fake network. Dropping it is the connection
/// dropping without a Leave.
pub struct Client {
    to_server: mpsc::UnboundedSender<Vec<u8>>,
    from_server: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl Client {
    pub fn send(&self, message: &ClientMessage) {
        // A closed session is what `recv` reports; a send into it is dropped.
        let _ = self.to_server.send(encode(message));
    }

    /// The next message from the server, or `None` once the server has
    /// ended the session or nothing arrives within the receive timeout.
    pub async fn recv(&mut self) -> Option<ServerMessage> {
        let bytes = tokio::time::timeout(RECEIVE_TIMEOUT, self.from_server.recv())
            .await
            .ok()??;
        Some(decode(&bytes).expect("the server sends only messages"))
    }

    /// Join `code` as `nickname` at this build's protocol version, answered
    /// with the slot and the room, or the refusal.
    pub async fn join(&mut self, code: &str, nickname: &str) -> Result<(u8, Room), Refusal> {
        self.join_as_version(code, nickname, PROTOCOL_VERSION).await
    }

    /// Join claiming `version`, the way a client from an older deploy does.
    pub async fn join_as_version(
        &mut self,
        code: &str,
        nickname: &str,
        version: u16,
    ) -> Result<(u8, Room), Refusal> {
        self.send(&ClientMessage::Join {
            version,
            room: code.to_owned(),
            nickname: nickname.to_owned(),
        });
        match self.recv().await {
            Some(ServerMessage::Joined { slot, room }) => Ok((slot, room)),
            Some(ServerMessage::Refused(refusal)) => Err(refusal),
            other => panic!("a Join is answered with Joined or Refused, not {other:?}"),
        }
    }

    /// The next Room message, skipping any other kind; `None` once the
    /// session has ended or nothing arrives in time.
    pub async fn next_room(&mut self) -> Option<Room> {
        loop {
            match self.recv().await? {
                ServerMessage::Room(room) => return Some(room),
                _ => continue,
            }
        }
    }

    /// Drop the connection without a Leave.
    pub fn disconnect(self) {
        drop(self);
    }
}

/// A one-way channel through the fake network: what goes in comes out
/// after the conditions' delay, in order, unless lost.
fn pump(
    conditions: Conditions,
    seed: u64,
) -> (mpsc::UnboundedSender<Vec<u8>>, mpsc::UnboundedReceiver<Vec<u8>>) {
    let (into, mut travelling) = mpsc::unbounded_channel::<Vec<u8>>();
    let (out, arrived) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut rng = StdRng::seed_from_u64(seed);
        while let Some(bytes) = travelling.recv().await {
            if conditions.loss > 0.0 && rng.random_bool(conditions.loss.min(1.0)) {
                continue;
            }
            let delay = one_way_delay(conditions, &mut rng);
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            if out.send(bytes).is_err() {
                return;
            }
        }
    });
    (into, arrived)
}

fn one_way_delay(conditions: Conditions, rng: &mut impl Rng) -> Duration {
    if conditions.jitter.is_zero() {
        return conditions.delay;
    }
    let jitter = conditions.jitter.as_secs_f64();
    let offset = rng.random_range(-jitter..=jitter);
    Duration::from_secs_f64((conditions.delay.as_secs_f64() + offset).max(0.0))
}
