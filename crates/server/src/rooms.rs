//! The rooms the server hosts: a registry by code and one tokio task per
//! room (ADR 0005). A session finds its room here, hands it a channel to
//! the client and forwards the client's messages; the task applies the
//! rules in [`crate::room`], tells every player what changed and closes the
//! room once it has stood empty for the tuned time (ADR 0012).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use protocol::{encode, ClientMessage, Refusal, Room, ServerMessage};
use rand::distr::slice::Choose;
use rand::Rng;
use sim::tuning::{ROOM_CLOSE_AFTER_EMPTY_S, ROOM_CODE_ALPHABET, ROOM_CODE_LENGTH};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::Instant;

use crate::room::{HostedRoom, Left};
use crate::Shutdown;

/// Bytes on their way to one client, whichever transport carries them.
pub type Outbound = mpsc::UnboundedSender<Vec<u8>>;

/// Every room on this server. Cheap to clone; every clone is the same
/// registry.
#[derive(Clone)]
pub struct Rooms {
    registry: Arc<Mutex<Registry>>,
    shutdown: Shutdown,
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
    /// An empty registry. Room tasks end when `shutdown` resolves.
    pub fn new(shutdown: Shutdown) -> Self {
        Self {
            registry: Arc::default(),
            shutdown,
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
        let room = HostedRoom::new(code.clone());
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

    /// Seat a client in the room with `code`. The code is read as the
    /// player typed it, in either case. On success the room has already
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

/// The room task: apply each command to the room, tell the players, and
/// close the room once it has been empty for the tuned time or the server
/// stops. Removes itself from the registry when it ends, so a closed code
/// reads as unknown.
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
    loop {
        let closing = async {
            match empty_since {
                Some(since) => tokio::time::sleep_until(since + close_after).await,
                None => std::future::pending().await,
            }
        };
        // Shutdown first: a stopping server does not report the sessions it
        // is itself ending as departures.
        let command = tokio::select! {
            biased;
            () = shutdown.wait() => break,
            () = closing => {
                room.close();
                let _ = publish.send(room.view());
                break;
            }
            command = inbox.recv() => match command {
                Some(command) => command,
                None => break,
            },
        };
        let before = room.view();
        // The joiner learns the room from Joined, not from the update too.
        let mut told_already = None;
        match command {
            Command::Join {
                version,
                nickname,
                outbound,
                reply,
            } => {
                let joined = room.join(version, &nickname);
                if let Ok(slot) = joined {
                    let _ = outbound.send(encode(&ServerMessage::Joined {
                        slot,
                        room: room.view(),
                    }));
                    players.insert(slot, outbound);
                    told_already = Some(slot);
                }
                let _ = reply.send(joined);
            }
            Command::Message { slot, message } => match message {
                ClientMessage::Ready { ready } => {
                    room.ready(slot, ready);
                }
                ClientMessage::Start => {
                    if let Some(started) = room.start(slot, rand::random()) {
                        broadcast(
                            &players,
                            &ServerMessage::Countdown {
                                seed: started.seed,
                                track: started.track,
                            },
                        );
                    }
                }
                ClientMessage::Leave => {
                    if room.leave(slot) != Left::Unknown {
                        players.remove(&slot);
                    }
                }
                // The session answers a Ping; a second Join and a race
                // input have no meaning in a room before the race (issue 17).
                ClientMessage::Ping { .. }
                | ClientMessage::Join { .. }
                | ClientMessage::Input(_) => {}
            },
            Command::Dropped { slot } => {
                if room.leave(slot) != Left::Unknown {
                    players.remove(&slot);
                }
            }
        }
        let after = room.view();
        if after != before {
            let message = ServerMessage::Room(after.clone());
            let bytes = encode(&message);
            for (slot, outbound) in &players {
                if Some(*slot) != told_already {
                    // A player whose session has gone is dropped by their own command.
                    let _ = outbound.send(bytes.clone());
                }
            }
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

fn broadcast(players: &HashMap<u8, Outbound>, message: &ServerMessage) {
    let bytes = encode(message);
    for outbound in players.values() {
        // A player whose session has gone is dropped by their own command.
        let _ = outbound.send(bytes.clone());
    }
}
