//! An in-process stand-in for the network: a server with its rooms and up
//! to a grid of clients, joined by channels that add delay, jitter and loss
//! on the way. The room and race tests run on it (ADR 0009, ADR 0012). Run
//! under tokio's paused clock, a five-minute wait or a whole race takes
//! only the CPU time of its ticks.
//!
//! Each direction has the two lanes of ADR 0011: the reliable lane keeps
//! its order and loses nothing, as a stream does; the datagram lane, which
//! inputs and snapshots ride, loses each message with the tuned
//! probability and delays each on its own, so a later one can overtake.

use std::path::Path;
use std::time::Duration;

use protocol::{
    decode, ClientMessage, Event, FinalStanding, Input, Refusal, Room, ServerMessage, Snapshot,
    PROTOCOL_VERSION,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::sync::{mpsc, watch};

use crate::lane::{Lane, Outgoing};
use crate::log::{Captured, Log};
use crate::room::Started;
use crate::rooms::Rooms;
use crate::session::{self, Channels};
use crate::Shutdown;
use sim::Picks;

/// What the fake network does to every message, in each direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    /// Base one-way delay.
    pub delay: Duration,
    /// Each message's delay varies by up to this much either way. The
    /// reliable lane keeps its order regardless; the datagram lane may
    /// reorder.
    pub jitter: Duration,
    /// Probability in 0..=1 that a datagram never arrives.
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

/// The race the fake transport's rooms run: the oval, which a bot laps in
/// a fraction of the time Bell Deer takes, so a test through a whole
/// race costs seconds of CPU rather than tens.
pub const TRACK: &str = "oval";

/// A server and the network to it. Dropping it stops the server. The
/// server logs into a buffer rather than stdout (ADR 0018), which
/// [`Harness::logs`] reads back.
pub struct Harness {
    rooms: Rooms,
    conditions: Conditions,
    shutdown: Shutdown,
    stop: watch::Sender<bool>,
    next_seed: u64,
    captured: Captured,
}

impl Harness {
    /// A server whose rooms race [`TRACK`].
    pub fn new(conditions: Conditions) -> Self {
        Self::on_track(conditions, TRACK)
    }

    /// A server whose rooms race `track`, their seeds drawn from the
    /// conditions' seed so a run repeats.
    pub fn on_track(conditions: Conditions, track: &'static str) -> Self {
        Self::build(conditions, track, None)
    }

    /// A server whose rooms race [`TRACK`] and whose Open rooms are kept
    /// under `dir` (ADR 0005): a second harness over the same directory
    /// is the server after a restart, with those rooms back.
    pub fn kept_in(conditions: Conditions, dir: &Path) -> Self {
        Self::build(conditions, TRACK, Some(dir))
    }

    fn build(conditions: Conditions, track: &'static str, kept_in: Option<&Path>) -> Self {
        let (stop, shutdown) = Shutdown::pair();
        let (log, captured) = Log::capture();
        let mut rooms = Rooms::seeded(shutdown.clone(), track, conditions.seed).logging_to(log);
        if let Some(dir) = kept_in {
            rooms = rooms
                .kept_in(dir)
                .expect("the kept rooms directory is usable");
        }
        Self {
            rooms,
            conditions,
            shutdown,
            stop,
            next_seed: conditions.seed,
            captured,
        }
    }

    /// The server's rooms, as the HTTP endpoint and the sessions see them.
    pub fn rooms(&self) -> &Rooms {
        &self.rooms
    }

    /// Every line the server has logged so far, parsed.
    pub fn logs(&self) -> Vec<serde_json::Value> {
        self.captured.lines()
    }

    /// Every line the server has logged so far, as stdout would carry it.
    pub fn log_text(&self) -> String {
        self.captured.text()
    }

    /// What `POST /api/rooms` does: a new room's code, every pick as
    /// before there were picks.
    pub fn create_room(&self) -> String {
        self.create_room_with(Picks::default())
    }

    /// What `POST /api/rooms` with `picks` does: a new room's code, its
    /// races run as `picks` says.
    pub fn create_room_with(&self, picks: Picks) -> String {
        self.rooms
            .create_with(None, picks, None)
            .expect("the harness stays under the cap")
    }

    /// A client connected to the server through the fake network, not yet
    /// in any room.
    pub fn connect(&mut self) -> Client {
        self.connect_under(self.conditions)
    }

    /// A client whose network to the server has its own `conditions`, the
    /// harness's other clients keeping theirs; the seed is the harness's.
    pub fn connect_under(&mut self, conditions: Conditions) -> Client {
        let (server_inbound_tx, server_inbound) = mpsc::unbounded_channel();
        let up = self.lanes(conditions, server_inbound_tx);
        let (from_server_tx, from_server) = mpsc::unbounded_channel();
        let down = self.lanes(conditions, from_server_tx);
        let (server_outbound, mut outgoing) = mpsc::unbounded_channel::<Outgoing>();
        tokio::spawn(async move {
            while let Some(message) = outgoing.recv().await {
                if down.send(message).is_err() {
                    return;
                }
            }
        });

        tokio::spawn(session::run(
            self.rooms.clone(),
            Channels {
                inbound: server_inbound,
                outbound: server_outbound,
            },
            self.shutdown.clone(),
        ));
        Client {
            to_server: up,
            from_server,
            token: None,
        }
    }

    /// Both lanes one way under `conditions`, ending in `into`.
    fn lanes(&mut self, conditions: Conditions, into: mpsc::UnboundedSender<Vec<u8>>) -> Lanes {
        Lanes {
            reliable: reliable_lane(conditions, self.next_seed(), into.clone()),
            datagram: datagram_lane(conditions, self.next_seed(), into),
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

/// The two lanes one way through the fake network.
struct Lanes {
    reliable: mpsc::UnboundedSender<Vec<u8>>,
    datagram: mpsc::UnboundedSender<Vec<u8>>,
}

impl Lanes {
    /// Fails once the far end is gone.
    fn send(&self, Outgoing { lane, bytes }: Outgoing) -> Result<(), ()> {
        match lane {
            Lane::Reliable => self.reliable.send(bytes).map_err(|_| ()),
            Lane::Datagram => self.datagram.send(bytes).map_err(|_| ()),
        }
    }
}

/// One client's end of the fake network. Dropping it is the connection
/// dropping without a Leave. The server's probes of the connection (ADR
/// 0018) come on their own clock, so the receives pass over them and
/// [`Client::next_ping`] is the one way to read one.
pub struct Client {
    to_server: Lanes,
    from_server: mpsc::UnboundedReceiver<Vec<u8>>,
    /// The session token the last `Joined` dealt.
    token: Option<String>,
}

impl Client {
    /// Send on the lane the message's kind rides (ADR 0011).
    pub fn send(&self, message: &ClientMessage) {
        // A closed session is what `recv` reports; a send into it is dropped.
        let _ = self.to_server.send(Outgoing::from_client(message));
    }

    /// One tick's controls, on the datagram lane.
    pub fn send_input(&self, input: Input) {
        self.send(&ClientMessage::Input(input));
    }

    /// The next message from the server but a probe, or `None` once the
    /// server has ended the session or nothing arrives within the receive
    /// timeout.
    pub async fn recv(&mut self) -> Option<ServerMessage> {
        loop {
            let message = self.recv_any().await?;
            if !matches!(message, ServerMessage::Ping { .. }) {
                return Some(message);
            }
        }
    }

    /// The next message from the server but a probe, if one has arrived,
    /// without waiting: what a client's tick loop reads between ticks.
    pub fn try_recv(&mut self) -> Option<ServerMessage> {
        loop {
            let bytes = self.from_server.try_recv().ok()?;
            let message = decode(&bytes).expect("the server sends only messages");
            if !matches!(message, ServerMessage::Ping { .. }) {
                return Some(message);
            }
        }
    }

    /// The nonce of the server's next probe, passing over every other
    /// message; `None` as [`Self::recv`] is.
    pub async fn next_ping(&mut self) -> Option<u32> {
        loop {
            if let ServerMessage::Ping { nonce } = self.recv_any().await? {
                return Some(nonce);
            }
        }
    }

    /// The next message of any kind, probes included.
    async fn recv_any(&mut self) -> Option<ServerMessage> {
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
        self.joined().await
    }

    /// Take back the seat `token` holds in `code`, answered with the slot
    /// and the room, or the refusal.
    pub async fn rejoin(&mut self, code: &str, token: &str) -> Result<(u8, Room), Refusal> {
        self.send(&ClientMessage::Rejoin {
            version: PROTOCOL_VERSION,
            room: code.to_owned(),
            token: token.to_owned(),
        });
        self.joined().await
    }

    /// The session token the server dealt this client, once it has joined.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    async fn joined(&mut self) -> Result<(u8, Room), Refusal> {
        match self.recv().await {
            Some(ServerMessage::Joined { slot, room, token }) => {
                self.token = Some(token);
                Ok((slot, room))
            }
            Some(ServerMessage::Refused(refusal)) => Err(refusal),
            other => panic!("a Join is answered with Joined or Refused, not {other:?}"),
        }
    }

    /// The next message `pick` accepts, skipping any other; `None` once the
    /// session has ended or nothing arrives in time.
    pub async fn next_where<T>(&mut self, pick: impl Fn(ServerMessage) -> Option<T>) -> Option<T> {
        loop {
            if let Some(picked) = pick(self.recv().await?) {
                return Some(picked);
            }
        }
    }

    /// The next Room message, skipping any other kind.
    pub async fn next_room(&mut self) -> Option<Room> {
        self.next_where(|message| match message {
            ServerMessage::Room(room) => Some(room),
            _ => None,
        })
        .await
    }

    /// The next Countdown, skipping any other kind, as the race the room
    /// started.
    pub async fn next_countdown(&mut self) -> Option<Started> {
        self.next_where(|message| match message {
            ServerMessage::Countdown {
                seed,
                track,
                picks,
                grid,
            } => Some(Started {
                seed,
                track,
                picks,
                grid,
            }),
            _ => None,
        })
        .await
    }

    /// The next Snapshot, skipping any other kind.
    pub async fn next_snapshot(&mut self) -> Option<Snapshot> {
        self.next_where(|message| match message {
            ServerMessage::Snapshot(snapshot) => Some(snapshot),
            _ => None,
        })
        .await
    }

    /// The next Events message, skipping any other kind.
    pub async fn next_events(&mut self) -> Option<Vec<Event>> {
        self.next_where(|message| match message {
            ServerMessage::Events(events) => Some(events),
            _ => None,
        })
        .await
    }

    /// The next Results, skipping any other kind.
    pub async fn next_results(&mut self) -> Option<Vec<FinalStanding>> {
        self.next_where(|message| match message {
            ServerMessage::Results { standings } => Some(standings),
            _ => None,
        })
        .await
    }

    /// Drop the connection without a Leave.
    pub fn disconnect(self) {
        drop(self);
    }
}

/// The reliable lane one way: what goes in comes out of `into` after the
/// conditions' delay, in order and never lost.
fn reliable_lane(
    conditions: Conditions,
    seed: u64,
    into: mpsc::UnboundedSender<Vec<u8>>,
) -> mpsc::UnboundedSender<Vec<u8>> {
    let (sender, mut travelling) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        let mut rng = StdRng::seed_from_u64(seed);
        while let Some(bytes) = travelling.recv().await {
            // Waiting here, before the next message is read, keeps the order.
            let delay = one_way_delay(conditions, &mut rng);
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            if into.send(bytes).is_err() {
                return;
            }
        }
    });
    sender
}

/// The datagram lane one way: each message is lost with the tuned
/// probability, and otherwise travels on its own, so one delayed more than
/// the next arrives after it.
fn datagram_lane(
    conditions: Conditions,
    seed: u64,
    into: mpsc::UnboundedSender<Vec<u8>>,
) -> mpsc::UnboundedSender<Vec<u8>> {
    let (sender, mut travelling) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        let mut rng = StdRng::seed_from_u64(seed);
        while let Some(bytes) = travelling.recv().await {
            if conditions.loss > 0.0 && rng.random_bool(conditions.loss.min(1.0)) {
                continue;
            }
            let delay = one_way_delay(conditions, &mut rng);
            if delay.is_zero() {
                if into.send(bytes).is_err() {
                    return;
                }
            } else {
                let into = into.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    let _ = into.send(bytes);
                });
            }
        }
    });
    sender
}

fn one_way_delay(conditions: Conditions, rng: &mut impl Rng) -> Duration {
    if conditions.jitter.is_zero() {
        return conditions.delay;
    }
    let jitter = conditions.jitter.as_secs_f64();
    let offset = rng.random_range(-jitter..=jitter);
    Duration::from_secs_f64((conditions.delay.as_secs_f64() + offset).max(0.0))
}
