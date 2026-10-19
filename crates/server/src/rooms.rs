//! The rooms the server hosts: a registry by code and one tokio task per
//! room (ADR 0005). A session finds its room here, hands it a channel to
//! the client and forwards the client's messages; the task applies the
//! rules in [`crate::room`], tells every player what changed, runs the
//! room's race in [`crate::race`] at the tick rate, and closes the room
//! once it has stood empty for the tuned time (ADR 0012).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use protocol::{ClientMessage, Refusal, Room, ServerMessage};
use rand::distr::slice::Choose;
use rand::Rng;
use sim::tuning::{ROOM_CLOSE_AFTER_EMPTY_S, ROOM_CODE_ALPHABET, ROOM_CODE_LENGTH};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Instant, Interval, MissedTickBehavior};

use crate::lane::{Outbound, Outgoing};
use crate::race::{Race, TICK_S};
use crate::room::{HostedRoom, Left};
use crate::{tracks, Shutdown};

/// Every room on this server. Cheap to clone; every clone is the same
/// registry.
#[derive(Clone)]
pub struct Rooms {
    registry: Arc<Mutex<Registry>>,
    shutdown: Shutdown,
    /// The track every room created here races.
    track: &'static str,
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
        reply: oneshot::Sender<Result<u8, Refusal>>,
    },
    /// A message from the player in `slot`.
    Message { slot: u8, message: ClientMessage },
    /// The player in `slot` lost their connection without a Leave.
    Dropped { slot: u8 },
}

/// A player's membership of a room, held by their session: the messages
/// they send go through it, and dropping it tells the room they are gone.
pub struct Membership {
    slot: u8,
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
            message,
        });
    }

    /// Leave the room on purpose; the membership is over.
    pub fn leave(mut self) {
        self.left = true;
        let _ = self.commands.send(Command::Message {
            slot: self.slot,
            message: ClientMessage::Leave,
        });
    }
}

impl Drop for Membership {
    fn drop(&mut self) {
        if !self.left {
            let _ = self.commands.send(Command::Dropped { slot: self.slot });
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
        let (commands, inbox) = mpsc::unbounded_channel();
        let room = HostedRoom::on_track(code.clone(), self.track);
        let (publish, view) = watch::channel(room.view());
        registry
            .rooms
            .insert(code.clone(), RoomHandle { commands, view });
        tokio::spawn(run(
            room,
            inbox,
            publish,
            Arc::downgrade(&self.registry),
            self.shutdown.clone(),
        ));
        code
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
        let code = code.trim().to_ascii_uppercase();
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
            .get(&code)
            .map(|handle| handle.commands.clone())
            .ok_or(Refusal::UnknownRoom)?;
        let (reply, answer) = oneshot::channel();
        commands
            .send(Command::Join {
                version,
                nickname,
                outbound,
                reply,
            })
            .map_err(|_| Refusal::UnknownRoom)?;
        // A room that closed between the lookup and the answer drops the reply.
        let slot = answer.await.map_err(|_| Refusal::UnknownRoom)??;
        Ok(Membership {
            slot,
            commands,
            left: false,
        })
    }
}

/// A code of the tuned length from the unambiguous alphabet.
fn fresh_code(rng: &mut impl Rng) -> String {
    let letters: Vec<char> = ROOM_CODE_ALPHABET.chars().collect();
    let letter = Choose::new(&letters).expect("the alphabet is not empty");
    rng.sample_iter(letter).take(ROOM_CODE_LENGTH).collect()
}

/// A race under way in a room task: the race, the clock that ticks it and
/// how long its ticks have taken.
struct Running {
    race: Race,
    ticks: Interval,
    timing: Timing,
}

/// What one tick did to the room, for the task to tell the players.
struct TickOutcome {
    /// The race ended: the results have been sent and the race is over.
    finished: bool,
}

impl Running {
    fn start(race: Race) -> Self {
        let mut ticks = tokio::time::interval(Duration::from_secs_f32(TICK_S));
        // A tick the server fell behind on is run as soon as it can be, so
        // the sim keeps its time; the interval only waits when ahead.
        ticks.set_missed_tick_behavior(MissedTickBehavior::Burst);
        Self {
            race,
            ticks,
            timing: Timing::default(),
        }
    }

    /// One tick: step the race, send what it produced to the players and
    /// move the room along with the race's phase. The whole of it is timed
    /// against the tick budget, the sending included.
    fn tick(&mut self, room: &mut HostedRoom, players: &mut HashMap<u8, Outbound>) -> TickOutcome {
        let started_at = Instant::now();
        let ticked = self.race.tick();
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
            room.finish();
            // A seat that freed at the finish stops receiving.
            let seated: Vec<u8> = room.view().players.iter().map(|player| player.slot).collect();
            players.retain(|slot, _| seated.contains(slot));
        }
        self.timing.record(started_at.elapsed());
        TickOutcome { finished }
    }
}

/// How long the ticks of a race took on the server, for the log line that
/// checks the budget until metrics land (ADR 0018).
#[derive(Debug, Clone, Copy, Default)]
struct Timing {
    ticks: u32,
    total: Duration,
    longest: Duration,
    /// Ticks whose work took longer than a tick.
    over_budget: u32,
}

impl Timing {
    fn record(&mut self, took: Duration) {
        self.ticks += 1;
        self.total += took;
        self.longest = self.longest.max(took);
        if took > Duration::from_secs_f32(TICK_S) {
            self.over_budget += 1;
        }
    }

    fn mean(&self) -> Duration {
        if self.ticks == 0 {
            Duration::ZERO
        } else {
            self.total / self.ticks
        }
    }

    /// The budget check until metrics land (ADR 0018): one line per race.
    fn log(&self, code: &str, ending: &str) {
        println!(
            "room {code} race {ending}: {} ticks, mean {:.2} ms, longest {:.2} ms, {} over the {:.1} ms budget",
            self.ticks,
            self.mean().as_secs_f64() * 1000.0,
            self.longest.as_secs_f64() * 1000.0,
            self.over_budget,
            f64::from(TICK_S) * 1000.0,
        );
    }
}

/// The room task: apply each command to the room, tick its race, tell the
/// players, and close the room once it has been empty for the tuned time
/// or the server stops. Removes itself from the registry when it ends, so
/// a closed code reads as unknown.
async fn run(
    mut room: HostedRoom,
    mut inbox: mpsc::UnboundedReceiver<Command>,
    publish: watch::Sender<Room>,
    registry: Weak<Mutex<Registry>>,
    shutdown: Shutdown,
) {
    let close_after = Duration::from_secs_f32(ROOM_CLOSE_AFTER_EMPTY_S);
    let mut players: HashMap<u8, Outbound> = HashMap::new();
    let mut empty_since = Some(Instant::now());
    let mut running: Option<Running> = None;
    // Event ids count up per room, across its races, so a client's
    // de-duplication never meets an id again (ADR 0011).
    let mut next_event_id = 1;
    loop {
        let closing = async {
            match empty_since {
                Some(since) => tokio::time::sleep_until(since + close_after).await,
                None => std::future::pending().await,
            }
        };
        let tick = async {
            match running.as_mut() {
                Some(running) => running.ticks.tick().await,
                None => std::future::pending().await,
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
                if let Some(running) = &running {
                    running.timing.log(room.code(), "cut short by the server stopping");
                }
                break;
            }
            () = closing => {
                room.close();
                let _ = publish.send(room.view());
                break;
            }
            _ = tick => {
                let outcome = running
                    .as_mut()
                    .expect("the tick arm only runs with a race")
                    .tick(&mut room, &mut players);
                if outcome.finished {
                    let finished = running.take().expect("the race that just finished");
                    next_event_id = finished.race.next_event_id();
                    finished.timing.log(room.code(), "over");
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
                        let joined = room.join(version, &nickname);
                        if let Ok(slot) = joined {
                            let _ = outbound.send(Outgoing::from_server(&ServerMessage::Joined {
                                slot,
                                room: room.view(),
                            }));
                            players.insert(slot, outbound);
                            told_already = Some(slot);
                        }
                        let _ = reply.send(joined);
                    }
                    Command::Message { slot, message } => match message {
                        ClientMessage::Input(input) => {
                            if let Some(running) = running.as_mut() {
                                running.race.receive(slot, input);
                            }
                        }
                        ClientMessage::Ready { ready } => {
                            room.ready(slot, ready);
                        }
                        ClientMessage::Start => {
                            if let Some(started) = room.start(slot, rand::random()) {
                                let track = tracks::load(&started.track)
                                    .expect("a room races a track the server has");
                                running = Some(Running::start(Race::new(
                                    started.seed,
                                    track,
                                    started.grid.clone(),
                                    next_event_id,
                                )));
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
                        ClientMessage::Leave => depart(&mut room, &mut players, running.as_mut(), slot),
                        // The session answers a Ping; a second Join has no
                        // meaning in a room.
                        ClientMessage::Ping { .. } | ClientMessage::Join { .. } => {}
                    },
                    Command::Dropped { slot } => depart(&mut room, &mut players, running.as_mut(), slot),
                }
            }
        }
        let after = room.view();
        if after != before {
            broadcast(&players, &ServerMessage::Room(after.clone()), told_already);
            let _ = publish.send(after);
        }
        empty_since = match (room.connected_count(), empty_since) {
            (0, None) => Some(Instant::now()),
            (0, since) => since,
            (_, _) => None,
        };
    }
    if let Some(registry) = registry.upgrade() {
        registry
            .lock()
            .expect("registry lock")
            .rooms
            .remove(room.code());
    }
}

/// The player leaves or drops; a seat that frees stops receiving, and a
/// kart whose player is gone mid-race holds no controls.
fn depart(
    room: &mut HostedRoom,
    players: &mut HashMap<u8, Outbound>,
    running: Option<&mut Running>,
    slot: u8,
) {
    match room.leave(slot) {
        Left::Unknown => {}
        Left::Removed => {
            players.remove(&slot);
        }
        Left::Disconnected => {
            players.remove(&slot);
            if let Some(running) = running {
                running.race.release(slot);
            }
        }
    }
}

/// Send `message` to every connected player but `except`.
fn broadcast(players: &HashMap<u8, Outbound>, message: &ServerMessage, except: Option<u8>) {
    let outgoing = Outgoing::from_server(message);
    for (slot, outbound) in players {
        if Some(*slot) != except {
            // A player whose session has gone is dropped by their own command.
            let _ = outbound.send(outgoing.clone());
        }
    }
}
