//! Wire messages between client and server, defined once here. The web app's
//! TypeScript types are generated from this crate by `examples/generate.rs`
//! (`pnpm generate:protocol`), never written by hand; CI fails when the
//! committed output is stale (ADR 0004).
//!
//! Every message is serialised with postcard (ADR 0004), which carries no
//! field names and no tags beyond a variant index, so every enum here is
//! externally tagged (postcard cannot decode the other serde taggings) and
//! the order of variants and fields is part of the wire format. The encoding
//! is the same whichever transport carries it (ADR 0011).
//!
//! The same serde definitions give the messages their JavaScript shape in
//! the bridge, so a TypeScript value of a generated type is a message.

pub mod samples;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// Bumped on every change to a message shape. The client sends it in
/// [`ClientMessage::Join`] and the server refuses a mismatch with
/// [`Refusal::VersionMismatch`] (ADR 0013). The generator exports it next to
/// the types, so the web app reads the same constant.
pub const PROTOCOL_VERSION: u16 = 5;

/// What a client sends the server.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientMessage {
    /// Ask for a [`ServerMessage::Pong`] carrying the same nonce; the M0(b)
    /// round trip that proves a transport.
    Ping { nonce: u32 },
    /// The first message on a connection: join the room with this code under
    /// this nickname, speaking this protocol version. Answered with
    /// [`ServerMessage::Joined`] or [`ServerMessage::Refused`].
    Join {
        version: u16,
        room: String,
        nickname: String,
    },
    /// One tick's controls, sent at the tick rate on a datagram (ADR 0010).
    Input(Input),
    /// Toggle the player's ready state in the room.
    Ready { ready: bool },
    /// Start the race; honoured only from the creator once every human is
    /// ready.
    Start,
    /// Back from the results to the room for another race: the player is
    /// Unready again and the room, if it was still showing Results, is Open
    /// (ADR 0012).
    Rematch,
    /// Leave the room on purpose, as opposed to dropping the connection.
    Leave,
    /// The first message on a connection that takes back a seat: the one
    /// [`ServerMessage::Joined`] gave this session token in the room with
    /// this code. Answered like a Join; mid-race the kart is the player's
    /// again and the Countdown follows (ADR 0012).
    Rejoin {
        version: u16,
        room: String,
        token: String,
    },
}

/// One kart's controls for one tick, quantised to a byte each so the message
/// stays near the eight-byte budget (ADR 0010).
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    /// Counts up by one per input sent; snapshots acknowledge the last one
    /// applied so the client knows which inputs to replay (ADR 0009).
    pub sequence: u32,
    /// -127 and below is full left, 127 is full right.
    pub steer: i8,
    /// 0 is off, 255 is full.
    pub throttle: u8,
    /// 0 is off, 255 is full.
    pub brake: u8,
    /// The hop button is down.
    pub hop: bool,
    /// The item button went down this tick.
    pub use_item: bool,
    /// The item button is held, which keeps a dropped hazard behind the kart.
    pub hold_item: bool,
}

impl Input {
    /// The steer in the unit range the sim reads: -1 is full left, 1 full
    /// right. The server and the client's prediction both read it here, so
    /// they step the same kart (ADR 0009).
    pub fn steer_unit(&self) -> f32 {
        (f32::from(self.steer) / f32::from(i8::MAX)).clamp(-1.0, 1.0)
    }

    /// The throttle in the unit range the sim reads.
    pub fn throttle_unit(&self) -> f32 {
        f32::from(self.throttle) / f32::from(u8::MAX)
    }

    /// The brake in the unit range the sim reads.
    pub fn brake_unit(&self) -> f32 {
        f32::from(self.brake) / f32::from(u8::MAX)
    }
}

/// What the server sends a client.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    /// The answer to a [`ClientMessage::Ping`] with that ping's nonce.
    Pong { nonce: u32 },
    /// The join was accepted: the player's slot, the room as it stands, and
    /// the session token a [`ClientMessage::Rejoin`] takes the seat back
    /// with, kept secret by the client. A rejoin keeps the token.
    Joined { slot: u8, room: Room, token: String },
    /// The join was refused for this reason; the connection then closes.
    Refused(Refusal),
    /// The room after a change to it: a player joined, left, readied or
    /// became the creator, or the room changed state.
    Room(Room),
    /// The race is starting: every client creates the sim from this seed on
    /// this track and runs the countdown in it (ADR 0002). `grid` is the
    /// room slot of each human kart in grid order, so a kart index in a
    /// snapshot or an event names a player through it; the karts behind
    /// the humans, up to the race's grid, are bots.
    Countdown {
        seed: u64,
        track: String,
        grid: Vec<u8>,
    },
    /// The authoritative state at a tick, on a datagram at the snapshot rate
    /// (ADR 0010).
    Snapshot(Snapshot),
    /// What happened this tick, on the reliable stream; the same events
    /// ride the next snapshot and the two after it, so a client keeps the
    /// first copy of each id it sees (ADR 0011).
    Events(Vec<Event>),
    /// The standings once the race is Finished, in finishing order.
    Results { standings: Vec<FinalStanding> },
    /// Another connection rejoined mid-race with this session's token, so
    /// this one hears nothing more; the client stops rather than rejoining
    /// back.
    Replaced,
}

/// Why a [`ClientMessage::Join`] was refused.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Refusal {
    /// No room has that code.
    UnknownRoom,
    /// The room already holds eight players.
    RoomFull,
    /// The room is between Countdown and Results; join after the race.
    RoomRacing,
    /// The client speaks another protocol version than the server; the
    /// player reloads to get the client that matches (ADR 0013).
    VersionMismatch { server: u16 },
    /// No seat in the room holds the rejoin's session token: the seat was
    /// freed, at the end of the race it was kept for or when its player
    /// left, or the token was never this room's.
    UnknownSession,
    /// Between races the seat's own connection still holds it, so the
    /// client joins as a new player instead: a second tab of one browser
    /// is a second player. Mid-race a rejoin takes the seat over.
    SessionInUse,
}

/// A room as every player in it sees it (ADR 0012).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    /// Four letters from the unambiguous alphabet.
    pub code: String,
    pub state: RoomState,
    /// In slot order. A joiner takes the lowest free slot, so a room nobody
    /// has left lists its players in join order.
    pub players: Vec<Player>,
}

/// The room state machine (ADR 0012).
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RoomState {
    Open,
    Countdown,
    Racing,
    Results,
    Closing,
}

/// One player in a room.
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    /// The player's seat in the room, kept for their whole stay. A race's
    /// Countdown says which kart the seat drives.
    pub slot: u8,
    pub nickname: String,
    /// Index into the eight liveries; the slot's, so it follows join order
    /// and stays with the player.
    pub livery: u8,
    pub state: PlayerState,
    /// Whether this player can press Start; exactly one player in a room is.
    pub creator: bool,
}

/// The player state machine within a room (ADR 0012).
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PlayerState {
    /// The session's state from its Join until the room answers; a seated
    /// player is at least Unready, so no `Player` on the wire is Joining.
    Joining,
    Unready,
    Ready,
    Racing,
    Finished,
    Disconnected,
}

/// The server's state of every kart at a tick, with what the client needs
/// to reconcile its prediction against it (ADR 0009).
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tick: u32,
    /// The input sequence applied at this tick, per kart in grid order; 0
    /// for a bot's kart and before a client's first input is applied. The
    /// server applies one sequence a tick, so the client knows the tick
    /// each later input meets (ADR 0019).
    pub acknowledged: Vec<u32>,
    /// One per kart in grid order.
    pub karts: Vec<KartSnapshot>,
    /// Since the previous snapshot, and repeated in the next two; the id
    /// de-duplicates a repeat (ADR 0011).
    pub events: Vec<Event>,
    /// Every hazard on the track at this tick, in the order they were fired.
    pub hazards: Vec<HazardSnapshot>,
}

/// A hazard on the track: where it lies, whose it is and whether that kart
/// is still trailing it, in which case the client draws it the tuned drop
/// distance behind the kart as it draws that kart.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HazardSnapshot {
    /// Its centre, on the surface, in metres.
    pub position: [f32; 3],
    /// The kart that fired it, in grid order.
    pub owner: u8,
    /// Whether that kart still trails it, so it moves with the kart.
    pub held: bool,
}

/// One kart's state in a snapshot. Metres, seconds and metres per second;
/// Y up; the rotation is a unit quaternion [x, y, z, w].
#[derive(Serialize, Deserialize, TS, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KartSnapshot {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    /// The drift under way, if any.
    pub drift: Option<DriftSnapshot>,
    /// Race position from 1, as the sim orders it.
    pub race_position: u8,
    /// The lap the kart is on, from 1.
    pub lap: u8,
    /// The last checkpoint crossed in order; 0 is the start line.
    pub checkpoint: u8,
    /// Distance along the centreline.
    pub spline_progress: f32,
    /// The wrong-way warning is up for this kart.
    pub wrong_way: bool,
    /// The item in the kart's slot, if any.
    pub held_item: Option<Item>,
    /// Seconds of spin-out left, while the kart is spun out.
    pub spin_out_s: Option<f32>,
}

/// A drift under way: its direction, fixed at the hop, and the seconds it
/// has been held, which is what the mini-turbo tier is read from.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DriftSnapshot {
    pub direction: DriftDirection,
    pub charge_s: f32,
}

/// Which way a drift turns the kart.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DriftDirection {
    Left,
    Right,
}

/// The items a kart can hold.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Item {
    /// A burst of speed.
    Boost,
    /// Dropped behind the kart; spins out whoever hits it.
    Hazard,
    /// Follows the track to the kart ahead and spins it out.
    Homing,
}

/// Something that happened on the server between two snapshots. Clients
/// only show it; the sim on the server decided it.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Counts up per room; a client that has seen this id drops the repeat.
    pub id: u32,
    pub kind: EventKind,
}

/// What an [`Event`] was. `kart` is the slot it happened to.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EventKind {
    /// `kart` picked up `item_box`, an index into the track's item boxes, which
    /// is gone until the tuned respawn, and rolled `item` into its slot.
    ItemPicked {
        kart: u8,
        item: Item,
        item_box: u8,
    },
    /// `kart` fired `item`; its slot is empty again.
    ItemFired {
        kart: u8,
        item: Item,
    },
    /// `kart` was spun out by an item `by` fired or dropped: stopped, with
    /// its inputs ignored, for the tuned time, which the snapshots carry.
    Hit {
        kart: u8,
        by: u8,
    },
    LapCompleted {
        kart: u8,
        lap: u8,
        lap_time_s: f32,
    },
    Finished {
        kart: u8,
        position: u8,
        finish_time_s: f32,
    },
    Respawned {
        kart: u8,
    },
}

/// One kart's place in the results.
#[derive(Serialize, Deserialize, TS, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FinalStanding {
    pub kart: u8,
    /// Race position from 1.
    pub position: u8,
    /// The race clock as the kart finished its laps; none when the race
    /// ended first and the kart was placed by progress.
    pub finish_time_s: Option<f32>,
    /// None when the kart completed no lap.
    pub best_lap_s: Option<f32>,
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
