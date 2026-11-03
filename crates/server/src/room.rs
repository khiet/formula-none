//! One room's rules (ADR 0012): who sits in it, which state it and each
//! player are in, and what each message from a player changes. Pure
//! bookkeeping with no clock and no channel, so the room task in `rooms`
//! decides when things happen and this module decides what.
//!
//! A player's slot is their seat: the lowest free one at join, kept until
//! they leave, and the livery and the kart index that go with it. Slots
//! never shift under a player, so the slot a client learns from `Joined`
//! names it for the whole stay. The seat also holds the session token the
//! join dealt, which takes it back after a lost connection: mid-race the
//! player is Disconnected and a bot drives their kart until they are back
//! or the race ends (ADR 0012). A seat a restart brought back is
//! Disconnected too, its player's state kept aside until their token
//! rejoins, so the room cannot start around an absent player (ADR 0053).

use protocol::{Player, PlayerState, Refusal, Room, RoomState, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use sim::tuning::{NICKNAME_MAX_CHARS, ROOM_PLAYERS_MAX};
use sim::BotLevel;

/// The room as the server holds it: the view every player gets plus the
/// order players joined in, which creator handover follows.
#[derive(Debug, Clone)]
pub struct HostedRoom {
    code: String,
    /// The track the room's races run on.
    track: &'static str,
    /// How hard the room's bots drive (ADR 0078).
    bot_level: BotLevel,
    state: RoomState,
    /// One entry per slot; `None` is a free seat.
    seats: Vec<Option<Seat>>,
    /// Counts up per join so the order is known even after seats are reused.
    joins: u64,
    /// The id the room's next race gives its first event: ids count up
    /// across the room's races, and across a restart, so a client's
    /// de-duplication never meets an id again (ADR 0011).
    next_event_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Seat {
    player: Player,
    joined_at: u64,
    token: String,
    /// The state the player had before a restart, while the seat waits
    /// Disconnected for their rejoin; never written to disk, so a second
    /// restart finds the state and not the wait.
    #[serde(skip)]
    waiting: Option<PlayerState>,
}

/// A room as it is written to disk between races (ADR 0005): everything
/// [`HostedRoom`] holds, the seats' tokens included, so the players rejoin
/// the room a restart brought back with the tokens they kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeptRoom {
    pub code: String,
    pub track: String,
    /// A room kept before there was a level comes back at Basic (ADR 0053).
    #[serde(default)]
    pub bot_level: BotLevel,
    pub state: RoomState,
    seats: Vec<Option<Seat>>,
    joins: u64,
    next_event_id: u32,
}

/// What a leave did to the room, so the task knows whether the kart stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Left {
    /// The player is gone from the room; the seat is free.
    Removed,
    /// Mid-race, so the kart stays for a bot to drive (ADR 0012): the
    /// player is Disconnected.
    Disconnected,
    /// No such player, or one already Disconnected.
    Unknown,
}

/// The race the creator started: what every client builds its sim from,
/// and the slot driving each human kart in grid order, so the karts are in
/// room order with the bots behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub seed: u64,
    pub track: String,
    pub bot_level: BotLevel,
    pub grid: Vec<u8>,
}

impl HostedRoom {
    /// An Open room with every seat free, racing `track`, one of
    /// [`crate::tracks`], against bots at `bot_level`.
    pub fn on_track(code: String, track: &'static str, bot_level: BotLevel) -> Self {
        Self {
            code,
            track,
            bot_level,
            state: RoomState::Open,
            seats: vec![None; ROOM_PLAYERS_MAX],
            joins: 0,
            next_event_id: 1,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    /// The room as it is kept on disk: a seat waiting for its player is
    /// written with the state it waits to give back.
    pub fn kept(&self) -> KeptRoom {
        let seats = self
            .seats
            .iter()
            .map(|seat| {
                seat.clone().map(|mut seat| {
                    if let Some(state) = seat.waiting.take() {
                        seat.player.state = state;
                    }
                    seat
                })
            })
            .collect();
        KeptRoom {
            code: self.code.clone(),
            track: self.track.to_owned(),
            bot_level: self.bot_level,
            state: self.state,
            seats,
            joins: self.joins,
            next_event_id: self.next_event_id,
        }
    }

    /// The room `kept` describes, racing `track`, one of
    /// [`crate::tracks`] picked by the caller; every seat is Disconnected
    /// until its token rejoins.
    pub fn from_kept(kept: KeptRoom, track: &'static str) -> Self {
        let seats = kept
            .seats
            .into_iter()
            .map(|seat| {
                seat.map(|mut seat| {
                    seat.waiting = Some(seat.player.state);
                    seat.player.state = PlayerState::Disconnected;
                    seat
                })
            })
            .collect();
        Self {
            code: kept.code,
            track,
            bot_level: kept.bot_level,
            state: kept.state,
            seats,
            joins: kept.joins,
            next_event_id: kept.next_event_id,
        }
    }

    /// The id the next race's first event gets.
    pub fn next_event_id(&self) -> u32 {
        self.next_event_id
    }

    /// The race just run spent the ids up to `next_event_id`; the next
    /// race continues from there.
    pub fn events_spent_through(&mut self, next_event_id: u32) {
        self.next_event_id = next_event_id;
    }

    /// Slots held by a player; the seats a restart brought back wait on
    /// these players' rejoins.
    pub fn slots(&self) -> Vec<u8> {
        self.players().map(|player| player.slot).collect()
    }

    /// The room as every player sees it: the players in slot order.
    pub fn view(&self) -> Room {
        Room {
            code: self.code.clone(),
            track: self.track.to_owned(),
            bot_level: self.bot_level,
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

    /// Seat a player holding session `token` and hand back their slot, or
    /// the reason they were refused. The version is checked first so an old
    /// client is told to reload whatever else is wrong (ADR 0013). The first
    /// player to join becomes the creator. The nickname is trimmed and cut
    /// to the tuned length; the client validates it before sending. The
    /// caller deals the token and must not deal one twice.
    pub fn join(&mut self, version: u16, nickname: &str, token: String) -> Result<u8, Refusal> {
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
            token,
            waiting: None,
        });
        Ok(slot as u8)
    }

    /// Hand back the seat `token` holds, the player Racing again if they
    /// were Disconnected mid-race and as they were if the seat waited for
    /// them through a restart; any other token is refused. Mid-race the
    /// seat is the new connection's even while `connected` says the old
    /// one holds it, since the server may not have noticed a lost one yet;
    /// between races that is refused, so a second tab joins as a new
    /// player.
    pub fn rejoin(&mut self, token: &str, connected: impl Fn(u8) -> bool) -> Result<u8, Refusal> {
        let seat = self
            .seats
            .iter_mut()
            .flatten()
            .find(|seat| seat.token == token)
            .ok_or(Refusal::UnknownSession)?;
        let slot = seat.player.slot;
        let racing = matches!(self.state, RoomState::Countdown | RoomState::Racing);
        if !racing && connected(slot) {
            return Err(Refusal::SessionInUse);
        }
        if let Some(state) = seat.waiting.take() {
            seat.player.state = state;
        } else {
            self.reconnect(slot);
        }
        Ok(slot)
    }

    /// The player's connection went mid-race: they are Disconnected and
    /// their kart is a bot's to drive, and the creator role moves on. True
    /// when the room changed; between races there is no kart to keep.
    pub fn disconnect(&mut self, slot: u8) -> bool {
        if !matches!(self.state, RoomState::Countdown | RoomState::Racing) {
            return false;
        }
        let Some(player) = self.player_mut(slot) else {
            return false;
        };
        if player.state != PlayerState::Racing {
            return false;
        }
        player.state = PlayerState::Disconnected;
        player.creator = false;
        self.hand_over_creator();
        true
    }

    /// The Disconnected player is back before the race ended: Racing
    /// again, though the creator role stays where it went. True when the
    /// room changed.
    pub fn reconnect(&mut self, slot: u8) -> bool {
        let Some(player) = self.player_mut(slot) else {
            return false;
        };
        if player.state != PlayerState::Disconnected {
            return false;
        }
        player.state = PlayerState::Racing;
        true
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
            track: self.track.to_owned(),
            bot_level: self.bot_level,
            grid: self.players().map(|player| player.slot).collect(),
        })
    }

    /// The countdown is over: the room is Racing. Nothing changes unless it
    /// was in Countdown.
    pub fn racing(&mut self) -> bool {
        if self.state != RoomState::Countdown {
            return false;
        }
        self.state = RoomState::Racing;
        true
    }

    /// The race is over: the room shows Results and every player who is
    /// still here is Finished. A seat whose player disconnected mid-race
    /// frees now (ADR 0012).
    pub fn finish(&mut self) {
        self.state = RoomState::Results;
        for seat in &mut self.seats {
            match seat {
                Some(current) if current.player.state == PlayerState::Disconnected => *seat = None,
                Some(current) => current.player.state = PlayerState::Finished,
                None => {}
            }
        }
        self.hand_over_creator();
    }

    /// The player is back from the results for another race: Unready, and
    /// the room is Open again if it was still showing Results, so the next
    /// race is started the way the first was. True when the room changed;
    /// a player who is not Finished has nothing to come back from.
    pub fn rematch(&mut self, slot: u8) -> bool {
        let Some(player) = self.player_mut(slot) else {
            return false;
        };
        if player.state != PlayerState::Finished {
            return false;
        }
        player.state = PlayerState::Unready;
        if self.state == RoomState::Results {
            self.state = RoomState::Open;
        }
        true
    }

    /// The player leaves, on purpose or by losing the connection. Between
    /// races the seat frees; mid-race the kart stays and the player is
    /// Disconnected. The creator role passes to the earliest-joined
    /// connected player left, so the room never gets stuck.
    pub fn leave(&mut self, slot: u8) -> Left {
        if self.player(slot).is_none() {
            return Left::Unknown;
        }
        match self.state {
            RoomState::Countdown | RoomState::Racing => {
                if self.disconnect(slot) {
                    Left::Disconnected
                } else {
                    Left::Unknown
                }
            }
            RoomState::Open | RoomState::Results | RoomState::Closing => {
                self.seats[usize::from(slot)] = None;
                self.hand_over_creator();
                Left::Removed
            }
        }
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
