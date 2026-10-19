//! One room's rules (ADR 0012): who sits in it, which state it and each
//! player are in, and what each message from a player changes. Pure
//! bookkeeping with no clock and no channel, so the room task in `rooms`
//! decides when things happen and this module decides what.
//!
//! A player's slot is their seat: the lowest free one at join, kept until
//! they leave, and the livery and the kart index that go with it. Slots
//! never shift under a player, so the slot a client learns from `Joined`
//! names it for the whole stay.

use protocol::{Player, PlayerState, Refusal, Room, RoomState, PROTOCOL_VERSION};
use sim::tuning::{NICKNAME_MAX_CHARS, ROOM_PLAYERS_MAX};

/// The track every room races until rooms pick one.
pub const TRACK: &str = "brands-patch";

/// The room as the server holds it: the view every player gets plus the
/// order players joined in, which creator handover follows.
#[derive(Debug, Clone)]
pub struct HostedRoom {
    code: String,
    state: RoomState,
    /// One entry per slot; `None` is a free seat.
    seats: Vec<Option<Seat>>,
    /// Counts up per join so the order is known even after seats are reused.
    joins: u64,
}

#[derive(Debug, Clone)]
struct Seat {
    player: Player,
    joined_at: u64,
}

/// What a leave did to the room, so the task knows whether the kart stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Left {
    /// The player is gone from the room; the seat is free.
    Removed,
    /// Mid-race, so the kart stays for a bot to drive (ADR 0012): the
    /// player is Disconnected.
    Disconnected,
    /// No such player.
    Unknown,
}

/// The race the creator started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub seed: u64,
    pub track: String,
}

impl HostedRoom {
    /// An Open room with every seat free.
    pub fn new(code: String) -> Self {
        Self {
            code,
            state: RoomState::Open,
            seats: vec![None; ROOM_PLAYERS_MAX],
            joins: 0,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    /// The room as every player sees it: the players in slot order.
    pub fn view(&self) -> Room {
        Room {
            code: self.code.clone(),
            state: self.state,
            players: self.players().cloned().collect(),
        }
    }

    /// Players who hold a connection; a Disconnected one does not. The room
    /// closes a while after this reaches zero.
    pub fn connected_count(&self) -> usize {
        self.players()
            .filter(|player| player.state != PlayerState::Disconnected)
            .count()
    }

    /// Seat a player and hand back their slot, or the reason they were
    /// refused. The version is checked first so an old client is told to
    /// reload whatever else is wrong (ADR 0013). The first player to join
    /// becomes the creator. The nickname is trimmed and cut to the tuned
    /// length; the client validates it before sending.
    pub fn join(&mut self, version: u16, nickname: &str) -> Result<u8, Refusal> {
        if version != PROTOCOL_VERSION {
            return Err(Refusal::VersionMismatch {
                server: PROTOCOL_VERSION,
            });
        }
        if matches!(self.state, RoomState::Countdown | RoomState::Racing) {
            return Err(Refusal::RoomRacing);
        }
        let slot = self
            .seats
            .iter()
            .position(Option::is_none)
            .ok_or(Refusal::RoomFull)?;
        let creator = self.players().all(|player| !player.creator);
        self.joins += 1;
        self.seats[slot] = Some(Seat {
            player: Player {
                slot: slot as u8,
                nickname: nickname.trim().chars().take(NICKNAME_MAX_CHARS).collect(),
                livery: slot as u8,
                state: PlayerState::Unready,
                creator,
            },
            joined_at: self.joins,
        });
        Ok(slot as u8)
    }

    /// Set the player's ready state; true when the room changed. Only an
    /// Unready or Ready player can toggle, so a racing player's late
    /// message changes nothing.
    pub fn ready(&mut self, slot: u8, ready: bool) -> bool {
        let Some(player) = self.player_mut(slot) else {
            return false;
        };
        let next = if ready {
            PlayerState::Ready
        } else {
            PlayerState::Unready
        };
        if !matches!(player.state, PlayerState::Unready | PlayerState::Ready)
            || player.state == next
        {
            return false;
        }
        player.state = next;
        true
    }

    /// Whether Start would be honoured now: the room is between races and
    /// every player is Ready. The creator's Start button follows this.
    pub fn can_start(&self) -> bool {
        matches!(self.state, RoomState::Open | RoomState::Results)
            && self
                .players()
                .all(|player| player.state == PlayerState::Ready)
            && self.players().next().is_some()
    }

    /// Start the race from `seed`: the room enters Countdown and every
    /// player Racing. Honoured only from the creator and only when
    /// [`Self::can_start`]; anything else returns `None` and changes nothing.
    pub fn start(&mut self, slot: u8, seed: u64) -> Option<Started> {
        let creator = self.player(slot).is_some_and(|player| player.creator);
        if !creator || !self.can_start() {
            return None;
        }
        self.state = RoomState::Countdown;
        for seat in self.seats.iter_mut().flatten() {
            seat.player.state = PlayerState::Racing;
        }
        Some(Started {
            seed,
            track: TRACK.to_owned(),
        })
    }

    /// The player leaves, on purpose or by losing the connection. Between
    /// races the seat frees; mid-race the kart stays and the player is
    /// Disconnected. The creator role passes to the earliest-joined
    /// connected player left, so the room never gets stuck.
    pub fn leave(&mut self, slot: u8) -> Left {
        let Some(seat) = self.seats.get_mut(usize::from(slot)) else {
            return Left::Unknown;
        };
        let Some(current) = seat.as_mut() else {
            return Left::Unknown;
        };
        let left = match self.state {
            RoomState::Countdown | RoomState::Racing => {
                if current.player.state == PlayerState::Disconnected {
                    return Left::Unknown;
                }
                current.player.state = PlayerState::Disconnected;
                current.player.creator = false;
                Left::Disconnected
            }
            RoomState::Open | RoomState::Results | RoomState::Closing => {
                *seat = None;
                Left::Removed
            }
        };
        self.hand_over_creator();
        left
    }

    /// Nobody has been here for the tuned time: the room is Closing and
    /// takes no more joins.
    pub fn close(&mut self) {
        self.state = RoomState::Closing;
    }

    fn hand_over_creator(&mut self) {
        if self.players().any(|player| player.creator) {
            return;
        }
        let next = self
            .seats
            .iter()
            .flatten()
            .filter(|seat| seat.player.state != PlayerState::Disconnected)
            .min_by_key(|seat| seat.joined_at)
            .map(|seat| seat.player.slot);
        if let Some(slot) = next {
            if let Some(player) = self.player_mut(slot) {
                player.creator = true;
            }
        }
    }

    fn players(&self) -> impl Iterator<Item = &Player> {
        self.seats.iter().flatten().map(|seat| &seat.player)
    }

    fn player(&self, slot: u8) -> Option<&Player> {
        self.seats
            .get(usize::from(slot))
            .and_then(|seat| seat.as_ref())
            .map(|seat| &seat.player)
    }

    fn player_mut(&mut self, slot: u8) -> Option<&mut Player> {
        self.seats
            .get_mut(usize::from(slot))
            .and_then(|seat| seat.as_mut())
            .map(|seat| &mut seat.player)
    }
}
