//! The rooms the server hosts: a registry by code and one tokio task per
//! room (ADR 0005). A session finds its room here, hands it a channel to
//! the client and forwards the client's messages; the task applies the
//! rules in [`crate::room`], tells every player what changed, runs the
//! room's race in [`crate::race`] at the tick rate, and closes the room
//! once it has stood empty for the tuned time (ADR 0012).
//!
//! A seat is reached through one connection at a time. A rejoin mid-race
//! with the seat's session token moves it to the new connection, and
//! whatever the old one sends or reports after that, its drop included, is
//! ignored.
//!
//! An Open room is kept on disk through [`crate::kept`] so a restart brings
//! it back (ADR 0005): its seats wait the tuned grace for their players'
//! rejoins, then free.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use protocol::{ClientMessage, Refusal, Room, RoomState, ServerMessage};
use rand::distr::slice::Choose;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sim::tuning::{
    CONNECTION_PROBE_INTERVAL_S, CONNECTION_PROBE_WINDOW, RESTORED_SEAT_GRACE_S,
    ROOM_CLOSE_AFTER_EMPTY_S, ROOM_CODE_ALPHABET, ROOM_CODE_LENGTH,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Instant, Interval, MissedTickBehavior};

use crate::kept::Store;
use crate::lane::{Outbound, Outgoing};
use crate::log::Log;
use crate::metrics::Metrics;
use crate::race::{Race, TICK_S};
use crate::room::{HostedRoom, Left, Started};
use crate::{tracks, Shutdown};

/// Every room on this server. Cheap to clone; every clone is the same
/// registry.
#[derive(Clone)]
pub struct Rooms {
    registry: Arc<Mutex<Registry>>,
    shutdown: Shutdown,
    /// The track every room created here races.
    track: &'static str,
    /// Where every race's seed comes from.
    seeds: RaceSeeds,
    /// What every room records into (ADR 0018).
    metrics: Metrics,
    /// Where every room's lines go (ADR 0018).
    log: Log,
    /// Where Open rooms are kept across a restart, when they are.
    store: Option<Store>,
}

/// The source of race seeds: the OS's entropy on the server, so no two
/// races are alike, or a seeded generator under the fake transport, so a
/// test's races, and the items they roll, repeat from run to run.
#[derive(Clone)]
enum RaceSeeds {
    Entropy,
    Seeded(Arc<Mutex<StdRng>>),
}

impl RaceSeeds {
    fn next(&self) -> u64 {
        match self {
            Self::Entropy => rand::random(),
            Self::Seeded(rng) => rng.lock().expect("seed lock").random(),
        }
    }
}

#[derive(Default)]
struct Registry {
    rooms: HashMap<String, RoomHandle>,
}

struct RoomHandle {
    commands: mpsc::UnboundedSender<Command>,
    /// The room as of its last change, for anyone outside the task.
    view: watch::Receiver<Room>,
}

/// What a session asks of a room task.
enum Command {
    Join {
        version: u16,
        nickname: String,
        outbound: Outbound,
        reply: oneshot::Sender<Result<Seated, Refusal>>,
    },
    /// Take back the seat `token` holds.
    Rejoin {
        token: String,
        outbound: Outbound,
        reply: oneshot::Sender<Result<Seated, Refusal>>,
    },
    /// A message from the player in `slot` over `connection`.
    Message {
        slot: u8,
        connection: u64,
        message: ClientMessage,
    },
    /// The player in `slot` lost `connection` without a Leave.
    Dropped { slot: u8, connection: u64 },
}

/// Where a join or rejoin put the session: the seat and the connection
/// that reaches it.
struct Seated {
    slot: u8,
    connection: u64,
}

/// A player's membership of a room, held by their session: the messages
/// they send go through it, and dropping it tells the room they are gone.
pub struct Membership {
    slot: u8,
    /// Which of the seat's connections this is; the room ignores one it has
    /// since moved the seat from.
    connection: u64,
    commands: mpsc::UnboundedSender<Command>,
    /// Set by [`Membership::leave`], so the drop does not report a second
    /// departure against a seat another player may hold by then.
    left: bool,
}

impl Membership {
    pub fn slot(&self) -> u8 {
        self.slot
    }

    /// Forward a message from the player; the room answers through the
    /// outbound channel it was given at the join.
    pub fn send(&self, message: ClientMessage) {
        // The room is gone when this fails; the session is about to learn so.
        let _ = self.commands.send(Command::Message {
            slot: self.slot,
            connection: self.connection,
            message,
        });
    }

    /// Leave the room on purpose; the membership is over.
    pub fn leave(mut self) {
        self.left = true;
        let _ = self.commands.send(Command::Message {
            slot: self.slot,
            connection: self.connection,
            message: ClientMessage::Leave,
        });
    }
}

impl Drop for Membership {
    fn drop(&mut self) {
        if !self.left {
            let _ = self.commands.send(Command::Dropped {
                slot: self.slot,
                connection: self.connection,
            });
        }
    }
}

impl Rooms {
    /// An empty registry whose rooms race the default track. Room tasks end
    /// when `shutdown` resolves.
    pub fn new(shutdown: Shutdown) -> Self {
        Self::on_track(shutdown, tracks::DEFAULT)
    }

    /// An empty registry whose rooms race `track`, which must be one the
    /// server has (the fake transport's tests race the oval, which is
    /// quicker).
    pub fn on_track(shutdown: Shutdown, track: &'static str) -> Self {
        assert!(tracks::sidecar(track).is_some(), "no track called {track}");
        Self {
            registry: Arc::default(),
            shutdown,
            track,
            seeds: RaceSeeds::Entropy,
            metrics: Metrics::new(),
            log: Log::stdout(),
            store: None,
        }
    }

    /// The same registry, logging to `log` instead of stdout.
    pub fn logging_to(self, log: Log) -> Self {
        Self { log, ..self }
    }

    /// The same registry, keeping every Open room under `dir` across a
    /// restart (ADR 0005): the rooms the last run kept there are brought
    /// back now, each holding its seats for the tuned grace, and from here
    /// on a room is written there while Open and removed once it leaves
    /// Open or closes. Must run inside a tokio runtime, which spawns the
    /// restored rooms' tasks; call it last, so they log where the rest do.
    pub fn kept_in(self, dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let store = Store::open(dir)?;
        let kept = store.load()?;
        let rooms = Self {
            store: Some(store),
            ..self
        };
        let mut registry = rooms.registry.lock().expect("registry lock");
        // A file that is not a room this server can host goes, with why.
        let drop_kept = |code: &str, reason: &str| {
            rooms
                .log
                .event("room_dropped")
                .room(code)
                .with("reason", reason)
                .emit();
            if let Some(store) = &rooms.store {
                let _ = store.forget(code);
            }
        };
        for room in kept {
            let room = match room {
                Ok(kept) => kept,
                Err(unreadable) => {
                    rooms
                        .log
                        .event("room_unreadable")
                        .with("path", unreadable.path.display().to_string())
                        .with("reason", unreadable.reason)
                        .emit();
                    continue;
                }
            };
            let code = room.code.clone();
            let Some(room) = HostedRoom::from_kept(room) else {
                drop_kept(&code, "unknown track");
                continue;
            };
            if room.view().state != RoomState::Open {
                drop_kept(&code, "not open");
                continue;
            }
            if registry.rooms.contains_key(&code) {
                drop_kept(&code, "code in use");
                continue;
            }
            rooms
                .log
                .event("room_restored")
                .room(&code)
                .with("slots", room.slots())
                .emit();
            rooms.spawn(&mut registry, room, true);
        }
        drop(registry);
        Ok(rooms)
    }

    /// What the rooms have recorded, for the scrape endpoint.
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Where the rooms log, for the rest of the server to log beside them.
    pub fn log(&self) -> &Log {
        &self.log
    }

    /// `on_track`, with every race seeded from `seed` in start order, so a
    /// run under the fake transport repeats.
    pub fn seeded(shutdown: Shutdown, track: &'static str, seed: u64) -> Self {
        Self {
            seeds: RaceSeeds::Seeded(Arc::new(Mutex::new(StdRng::seed_from_u64(seed)))),
            ..Self::on_track(shutdown, track)
        }
    }

    /// Open a room under a fresh code and hand the code back; the HTTP
    /// create endpoint is this. The room starts empty, so its closing clock
    /// is already running: a code nobody joins expires like one everybody
    /// left. Must run inside a tokio runtime, which spawns the room's task.
    pub fn create(&self) -> String {
        let mut registry = self.registry.lock().expect("registry lock");
        let code = loop {
            let code = fresh_code(&mut rand::rng());
            if !registry.rooms.contains_key(&code) {
                break code;
            }
        };
        let room = HostedRoom::on_track(code.clone(), self.track);
        self.log.event("room_created").room(&code).emit();
        self.spawn(&mut registry, room, false);
        code
    }

    /// Put `room` in `registry` and start its task; `restored` when a
    /// restart brought it back, so its seats wait for their players.
    fn spawn(&self, registry: &mut Registry, room: HostedRoom, restored: bool) {
        let (commands, inbox) = mpsc::unbounded_channel();
        let (publish, view) = watch::channel(room.view());
        registry
            .rooms
            .insert(room.code().to_owned(), RoomHandle { commands, view });
        tokio::spawn(run(
            room,
            inbox,
            publish,
            Arc::downgrade(&self.registry),
            Shared {
                shutdown: self.shutdown.clone(),
                seeds: self.seeds.clone(),
                metrics: self.metrics.clone(),
                log: self.log.clone(),
                store: self.store.clone(),
            },
            restored,
        ));
    }

    /// The room with this code as of its last change, or `None` for a code
    /// no room has, including a room that has closed.
    pub fn view(&self, code: &str) -> Option<Room> {
        self.registry
            .lock()
            .expect("registry lock")
            .rooms
            .get(code)
            .map(|handle| handle.view.borrow().clone())
    }

    /// Follow the room with this code through every change; the last value
    /// a closed room publishes has it Closing.
    pub fn watch(&self, code: &str) -> Option<watch::Receiver<Room>> {
        self.registry
            .lock()
            .expect("registry lock")
            .rooms
            .get(code)
            .map(|handle| handle.view.clone())
    }

    /// Codes of the rooms open right now.
    pub fn codes(&self) -> Vec<String> {
        self.registry
            .lock()
            .expect("registry lock")
            .rooms
            .keys()
            .cloned()
            .collect()
    }

    /// Seat a client in the room with `code`, read in either case with the
    /// spaces around it ignored. On success the room has already
    /// sent `Joined` through `outbound` and will send every later change
    /// there; the membership forwards the player's messages.
    pub async fn join(
        &self,
        code: &str,
        version: u16,
        nickname: String,
        outbound: Outbound,
    ) -> Result<Membership, Refusal> {
        self.seat(code, version, |reply| Command::Join {
            version,
            nickname,
            outbound,
            reply,
        })
        .await
    }

    /// Take back the seat `token` holds in the room with `code` over a new
    /// connection, as [`Self::join`] seats a new player. Mid-race the
    /// player's kart is theirs again and the room sends the Countdown after
    /// `Joined`, so a client that lost the race can rebuild it (ADR 0012).
    pub async fn rejoin(
        &self,
        code: &str,
        version: u16,
        token: String,
        outbound: Outbound,
    ) -> Result<Membership, Refusal> {
        self.seat(code, version, |reply| Command::Rejoin {
            token,
            outbound,
            reply,
        })
        .await
    }

    /// Send the room with `code` the command `ask` makes and wait for the
    /// seat it answers with; a refusal is logged against the code.
    async fn seat(
        &self,
        code: &str,
        version: u16,
        ask: impl FnOnce(oneshot::Sender<Result<Seated, Refusal>>) -> Command,
    ) -> Result<Membership, Refusal> {
        let code = code.trim().to_ascii_uppercase();
        self.seat_in(&code, version, ask)
            .await
            .inspect_err(|refusal| {
                self.log
                    .event("join_refused")
                    .room(&code)
                    .with("refusal", refusal)
                    .emit();
            })
    }

    async fn seat_in(
        &self,
        code: &str,
        version: u16,
        ask: impl FnOnce(oneshot::Sender<Result<Seated, Refusal>>) -> Command,
    ) -> Result<Membership, Refusal> {
        if version != protocol::PROTOCOL_VERSION {
            return Err(Refusal::VersionMismatch {
                server: protocol::PROTOCOL_VERSION,
            });
        }
        let commands = self
            .registry
            .lock()
            .expect("registry lock")
            .rooms
            .get(code)
            .map(|handle| handle.commands.clone())
            .ok_or(Refusal::UnknownRoom)?;
        let (reply, answer) = oneshot::channel();
        commands
            .send(ask(reply))
            .map_err(|_| Refusal::UnknownRoom)?;
        // A room that closed between the lookup and the answer drops the reply.
        let Seated { slot, connection } = answer.await.map_err(|_| Refusal::UnknownRoom)??;
        Ok(Membership {
            slot,
            connection,
            commands,
            left: false,
        })
    }
}

/// A session token: 128 random bits in hex, which nobody guesses into
/// another player's seat.
fn fresh_token(rng: &mut impl Rng) -> String {
    format!("{:032x}", rng.random::<u128>())
}

/// The connection a seat is reached through now, and how the server's
/// probes of it have fared (ADR 0018).
struct Connection {
    id: u64,
    outbound: Outbound,
    /// The probe sent last and not answered yet.
    probe: Option<Probe>,
    /// Whether each of the latest probes was answered in time, newest last.
    outcomes: VecDeque<bool>,
}

/// A Ping on its way to the client, or on its way back.
struct Probe {
    nonce: u32,
    sent_at: Instant,
}

impl Connection {
    fn new(id: u64, outbound: Outbound) -> Self {
        Self {
            id,
            outbound,
            probe: None,
            outcomes: VecDeque::with_capacity(CONNECTION_PROBE_WINDOW),
        }
    }

    /// Send the next probe, with `nonce`; one still outstanding was lost.
    fn probe(&mut self, nonce: u32) {
        if self.probe.take().is_some() {
            self.settle(false);
        }
        let _ = self
            .outbound
            .send(Outgoing::from_server(&ServerMessage::Ping { nonce }));
        self.probe = Some(Probe {
            nonce,
            sent_at: Instant::now(),
        });
    }

    /// The client's Pong: the round trip, when it answers the outstanding
    /// probe. A Pong for an older probe, which already counted as lost,
    /// changes nothing.
    fn pong(&mut self, nonce: u32) -> Option<Duration> {
        let probe = self.probe.take_if(|probe| probe.nonce == nonce)?;
        self.settle(true);
        Some(probe.sent_at.elapsed())
    }

    fn settle(&mut self, in_time: bool) {
        if self.outcomes.len() == CONNECTION_PROBE_WINDOW {
            self.outcomes.pop_front();
        }
        self.outcomes.push_back(in_time);
    }

    /// The share in 0..=1 of the settled probes in the window that were
    /// lost; 0 before any has settled.
    fn packet_loss(&self) -> f64 {
        if self.outcomes.is_empty() {
            return 0.0;
        }
        let lost = self.outcomes.iter().filter(|&&in_time| !in_time).count();
        lost as f64 / self.outcomes.len() as f64
    }
}

/// A code of the tuned length from the unambiguous alphabet.
fn fresh_code(rng: &mut impl Rng) -> String {
    let letters: Vec<char> = ROOM_CODE_ALPHABET.chars().collect();
    let letter = Choose::new(&letters).expect("the alphabet is not empty");
    rng.sample_iter(letter).take(ROOM_CODE_LENGTH).collect()
}

/// A race under way in a room task: the race and how it started, and the
/// clock that ticks it.
struct Running {
    race: Race,
    /// Sent again to a player who rejoins mid-race.
    started: Started,
    ticks: Interval,
}

/// What one tick did to the room, for the task to act on and log.
struct TickOutcome {
    /// The countdown ended this tick: the room is Racing.
    go: bool,
    /// The results went out this tick; the task retires the race.
    finished: bool,
    /// Humans the race stopped hearing from this tick.
    silenced: Vec<u8>,
}

impl Running {
    fn start(race: Race, started: Started) -> Self {
        let mut ticks = tokio::time::interval(Duration::from_secs_f32(TICK_S));
        // Ticks the server fell behind on run back to back, so the sim keeps
        // its time instead of skipping ticks.
        ticks.set_missed_tick_behavior(MissedTickBehavior::Burst);
        Self {
            race,
            started,
            ticks,
        }
    }

    /// One tick: step the race, send what it produced to the players and
    /// move the room along with the race's phase. Timed for the tick
    /// duration metric (ADR 0018) from before the step until the
    /// broadcasts are queued; the transports send them later.
    fn tick(
        &mut self,
        room: &mut HostedRoom,
        players: &mut HashMap<u8, Connection>,
        metrics: &Metrics,
    ) -> TickOutcome {
        let started_at = Instant::now();
        let ticked = self.race.tick();
        // A silent player keeps their connection: their next input is them
        // back, and a client that hears nothing reconnects by itself.
        for &slot in &ticked.silenced {
            room.disconnect(slot);
        }
        if !ticked.events.is_empty() {
            broadcast(players, &ServerMessage::Events(ticked.events), None);
        }
        if let Some(snapshot) = ticked.snapshot {
            broadcast(players, &ServerMessage::Snapshot(snapshot), None);
        }
        if ticked.go {
            room.racing();
        }
        let finished = ticked.results.is_some();
        if let Some(standings) = ticked.results {
            broadcast(players, &ServerMessage::Results { standings }, None);
            // Silent is not gone: a player whose connection still holds is
            // told the results and keeps the seat a Disconnected one loses.
            for &slot in players.keys() {
                room.reconnect(slot);
            }
            room.finish();
            // A seat that freed at the finish stops receiving.
            let seated: Vec<u8> = room
                .view()
                .players
                .iter()
                .map(|player| player.slot)
                .collect();
            players.retain(|slot, _| seated.contains(slot));
        }
        metrics.tick(started_at.elapsed());
        TickOutcome {
            go: ticked.go,
            finished,
            silenced: ticked.silenced,
        }
    }
}

/// What every room task shares with the registry that spawned it.
struct Shared {
    shutdown: Shutdown,
    seeds: RaceSeeds,
    metrics: Metrics,
    log: Log,
    store: Option<Store>,
}

/// The room task: apply each command to the room, tick its race, tell the
/// players, and close the room once it has been empty for the tuned time
/// or the server stops. Removes itself from the registry when it ends, so
/// a closed code reads as unknown. While Open the room is kept in the
/// store, if there is one, so a restart brings it back; `restored` says
/// this task is such a room, whose seats wait the tuned grace for their
/// players' rejoins.
async fn run(
    mut room: HostedRoom,
    mut inbox: mpsc::UnboundedReceiver<Command>,
    publish: watch::Sender<Room>,
    registry: Weak<Mutex<Registry>>,
    shared: Shared,
    restored: bool,
) {
    let Shared {
        shutdown,
        seeds,
        metrics,
        log,
        store,
    } = shared;
    let close_after = Duration::from_secs_f32(ROOM_CLOSE_AFTER_EMPTY_S);
    let mut players: HashMap<u8, Connection> = HashMap::new();
    let mut connections = 0..;
    let mut empty_since = Some(Instant::now());
    let mut running: Option<Running> = None;
    // The restored seats free at this instant unless reclaimed by then.
    let mut reclaim_by =
        restored.then(|| Instant::now() + Duration::from_secs_f32(RESTORED_SEAT_GRACE_S));
    // Event ids count up per room, across its races, so a client's
    // de-duplication never meets an id again (ADR 0011).
    let mut next_event_id = 1;
    // Probes of every connection, at the tuned interval while anyone is
    // seated; the intervals missed while nobody was are not made up, so a
    // joiner's first probes are not taken for lost ones.
    let mut probes = tokio::time::interval(Duration::from_secs_f32(CONNECTION_PROBE_INTERVAL_S));
    probes.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut nonces = 0u32..;
    let keep = |room: &HostedRoom| {
        let Some(store) = &store else { return };
        let result = if room.view().state == RoomState::Open {
            store.save(&room.kept())
        } else {
            store.forget(room.code())
        };
        if let Err(error) = result {
            log.event("room_keep_failed")
                .room(room.code())
                .with("error", error.to_string())
                .emit();
        }
    };
    // A new room is kept from its creation, so a restart under the
    // creator's join still has a room for the code they were given.
    if !restored {
        keep(&room);
    }
    // Whether the task ends because the server is stopping, which keeps the
    // room on disk, rather than because the room closed, which does not.
    let mut stopping = false;
    loop {
        let closing = async {
            match empty_since {
                Some(since) => tokio::time::sleep_until(since + close_after).await,
                None => std::future::pending().await,
            }
        };
        let reclaim = async {
            match reclaim_by {
                Some(by) => tokio::time::sleep_until(by).await,
                None => std::future::pending().await,
            }
        };
        let tick = async {
            match running.as_mut() {
                Some(running) => running.ticks.tick().await,
                None => std::future::pending().await,
            }
        };
        let probe = async {
            if players.is_empty() {
                std::future::pending().await
            } else {
                probes.tick().await
            }
        };
        let before = room.view();
        // The joiner learns the room from Joined, not from the update too.
        let mut told_already = None;
        // Shutdown first: a stopping server does not report the sessions it
        // is itself ending as departures.
        tokio::select! {
            biased;
            () = shutdown.wait() => {
                stopping = true;
                break;
            }
            () = closing => {
                room.close();
                let _ = publish.send(room.view());
                log.event("room_closed").room(room.code()).emit();
                break;
            }
            () = reclaim => {
                reclaim_by = None;
                for slot in room.slots() {
                    if !players.contains_key(&slot) {
                        room.leave(slot);
                        log.event("player_left")
                            .room(room.code())
                            .slot(slot)
                            .with("how", "unclaimed")
                            .emit();
                    }
                }
            }
            _ = tick => {
                let outcome = running
                    .as_mut()
                    .expect("the tick arm only runs with a race")
                    .tick(&mut room, &mut players, &metrics);
                let ticks = running.as_ref().expect("the race that just ticked").race.ticks();
                for slot in outcome.silenced {
                    log.event("player_silent").room(room.code()).slot(slot).tick(ticks).emit();
                }
                if outcome.go {
                    log.event("race_go").room(room.code()).tick(ticks).emit();
                }
                if outcome.finished {
                    let finished = running.take().expect("the race that just finished");
                    next_event_id = finished.race.next_event_id();
                    log.event("race_finished").room(room.code()).tick(ticks).emit();
                }
            }
            _ = probe => {
                for (&slot, connection) in players.iter_mut() {
                    connection.probe(nonces.next().expect("nonces never run out"));
                    metrics.packet_loss(room.code(), slot, connection.packet_loss());
                }
            }
            command = inbox.recv() => {
                let Some(command) = command else { break };
                match command {
                    Command::Join {
                        version,
                        nickname,
                        outbound,
                        reply,
                    } => {
                        let token = fresh_token(&mut rand::rng());
                        let joined = room.join(version, &nickname, token.clone()).map(|slot| {
                            let id = connections.next().expect("connection ids never run out");
                            seat(&room, &mut players, slot, token, Connection::new(id, outbound));
                            told_already = Some(slot);
                            log.event("player_joined")
                                .room(room.code())
                                .slot(slot)
                                .with("nickname", &nickname)
                                .emit();
                            Seated { slot, connection: id }
                        });
                        let _ = reply.send(joined);
                    }
                    Command::Rejoin {
                        token,
                        outbound,
                        reply,
                    } => {
                        let rejoined = room
                            .rejoin(&token, |slot| players.contains_key(&slot))
                            .map(|slot| {
                            let id = connections.next().expect("connection ids never run out");
                            let countdown = running.as_mut().map(|running| {
                                running.race.hand_back(slot);
                                let Started { seed, track, grid } = running.started.clone();
                                ServerMessage::Countdown { seed, track, grid }
                            });
                            let connection = Connection::new(id, outbound);
                            let new = connection.outbound.clone();
                            let old = seat(&room, &mut players, slot, token, connection);
                            if let Some(countdown) = countdown {
                                let _ = new.send(Outgoing::from_server(&countdown));
                            }
                            if let Some(old) = old {
                                let _ = old
                                    .outbound
                                    .send(Outgoing::from_server(&ServerMessage::Replaced));
                            }
                            told_already = Some(slot);
                            let line = log.event("player_rejoined").room(room.code()).slot(slot);
                            match running.as_ref() {
                                Some(running) => line.tick(running.race.ticks()).emit(),
                                None => line.emit(),
                            }
                            Seated { slot, connection: id }
                        });
                        let _ = reply.send(rejoined);
                    }
                    Command::Message { slot, connection, .. }
                    | Command::Dropped { slot, connection }
                        if players.get(&slot).map(|current| current.id) != Some(connection) => {}
                    Command::Message { slot, message, .. } => match message {
                        ClientMessage::Input(input) => {
                            if let Some(running) = running.as_mut() {
                                // Heard from again after falling silent.
                                if room.reconnect(slot) {
                                    running.race.hand_back(slot);
                                    log.event("player_heard")
                                        .room(room.code())
                                        .slot(slot)
                                        .tick(running.race.ticks())
                                        .emit();
                                }
                                running.race.receive(slot, input);
                            }
                        }
                        ClientMessage::Ready { ready } => {
                            room.ready(slot, ready);
                        }
                        ClientMessage::Start => {
                            if let Some(started) = room.start(slot, seeds.next()) {
                                let track = tracks::load(&started.track)
                                    .expect("a room races a track the server has");
                                log.event("race_started")
                                    .room(room.code())
                                    .slot(slot)
                                    .with("seed", started.seed)
                                    .with("track", &started.track)
                                    .with("grid", &started.grid)
                                    .emit();
                                running = Some(Running::start(
                                    Race::new(
                                        started.seed,
                                        track,
                                        started.grid.clone(),
                                        next_event_id,
                                    ),
                                    started.clone(),
                                ));
                                broadcast(
                                    &players,
                                    &ServerMessage::Countdown {
                                        seed: started.seed,
                                        track: started.track,
                                        grid: started.grid,
                                    },
                                    None,
                                );
                            }
                        }
                        ClientMessage::Rematch => {
                            room.rematch(slot);
                        }
                        ClientMessage::Leave => {
                            depart(&mut room, &mut players, running.as_mut(), slot, &log, "leave");
                        }
                        ClientMessage::Pong { nonce } => {
                            if let Some(connection) = players.get_mut(&slot) {
                                if let Some(round_trip) = connection.pong(nonce) {
                                    metrics.rtt(room.code(), slot, round_trip);
                                    metrics.packet_loss(room.code(), slot, connection.packet_loss());
                                }
                            }
                        }
                        ClientMessage::RaceReport { corrections_per_second } => {
                            metrics.corrections_per_second(room.code(), slot, corrections_per_second);
                            log.event("race_report")
                                .room(room.code())
                                .slot(slot)
                                .with("corrections_per_second", corrections_per_second)
                                .emit();
                        }
                        // The session answers a Ping; a second Join or
                        // Rejoin has no meaning in a room.
                        ClientMessage::Ping { .. }
                        | ClientMessage::Join { .. }
                        | ClientMessage::Rejoin { .. } => {}
                    },
                    Command::Dropped { slot, .. } => {
                        depart(&mut room, &mut players, running.as_mut(), slot, &log, "dropped");
                    }
                }
            }
        }
        let after = room.view();
        if after != before {
            broadcast(&players, &ServerMessage::Room(after.clone()), told_already);
            metrics.room_players(room.code(), after.players.len());
            let _ = publish.send(after);
            keep(&room);
        }
        empty_since = match (room.connected_count(), empty_since) {
            (0, None) => Some(Instant::now()),
            (0, since) => since,
            (_, _) => None,
        };
    }
    metrics.room_players(room.code(), 0);
    if !stopping {
        if let Some(store) = &store {
            let _ = store.forget(room.code());
        }
    }
    if let Some(registry) = registry.upgrade() {
        registry
            .lock()
            .expect("registry lock")
            .rooms
            .remove(room.code());
    }
}

/// Make `connection` the one that reaches `slot`, telling it `Joined` with
/// `token`; the connection it replaces, if any, is handed back.
fn seat(
    room: &HostedRoom,
    players: &mut HashMap<u8, Connection>,
    slot: u8,
    token: String,
    connection: Connection,
) -> Option<Connection> {
    let _ = connection
        .outbound
        .send(Outgoing::from_server(&ServerMessage::Joined {
            slot,
            room: room.view(),
            token,
        }));
    players.insert(slot, connection)
}

/// The player leaves or drops, `how` saying which for the log: they stop
/// receiving, and mid-race a bot drives their kart; a player already
/// handed to a bot for falling silent only stops receiving.
fn depart(
    room: &mut HostedRoom,
    players: &mut HashMap<u8, Connection>,
    running: Option<&mut Running>,
    slot: u8,
    log: &Log,
    how: &str,
) {
    players.remove(&slot);
    let left = room.leave(slot);
    if left != Left::Unknown {
        let line = log
            .event("player_left")
            .room(room.code())
            .slot(slot)
            .with("how", how);
        match running.as_ref() {
            Some(running) => line.tick(running.race.ticks()).emit(),
            None => line.emit(),
        }
    }
    if let (Left::Disconnected, Some(running)) = (left, running) {
        running.race.hand_to_bot(slot);
    }
}

/// Send `message` to every connected player but `except`.
fn broadcast(players: &HashMap<u8, Connection>, message: &ServerMessage, except: Option<u8>) {
    let outgoing = Outgoing::from_server(message);
    for (slot, connection) in players {
        if Some(*slot) != except {
            // A player whose session has gone is dropped by their own command.
            let _ = connection.outbound.send(outgoing.clone());
        }
    }
}
