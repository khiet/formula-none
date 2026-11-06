//! One room's race on the server, the only place its outcome is decided
//! (ADR 0009): the sim stepped from each human's inputs a sequence a tick,
//! the snapshots and events the players are told, and the results once it
//! is over. Pure bookkeeping with no clock and no channel: the room task in
//! `rooms` decides when a tick happens and where its output goes; this
//! module decides what a tick produces.

use std::collections::{BTreeMap, VecDeque};

use protocol::{
    Event, EventKind, FinalStanding, HazardSnapshot, Input, ProjectileSnapshot, Snapshot,
};
use sim::tuning::{
    CONNECTION_SILENCE_S, INPUT_QUEUE_MAX_TICKS, INPUT_QUEUE_START_TICKS, KARTS_PER_RACE,
    SNAPSHOTS_PER_SECOND, SNAPSHOT_EVENT_REPEATS, TICKS_PER_SECOND,
};
use sim::{BotLevel, KartInput, Phase, State, Track, Tyres};

/// The sim's `dt`, and the budget a tick's work must fit in for the server
/// to keep up (ADR 0010).
pub use sim::tuning::TICK_S;
const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;

/// A race in progress: the sim and what the players have sent it.
pub struct Race {
    /// `sim::step` consumes and returns the state; `None` only inside `tick`.
    state: Option<State>,
    /// The room slot driving each human kart, in grid order.
    grid: Vec<u8>,
    /// The latest input from each human, in grid order.
    humans: Vec<HumanInput>,
    /// Ticks stepped so far: the sim's clock as snapshots report it.
    tick: u32,
    next_event_id: u32,
    /// Events since the last snapshot.
    pending: Vec<Event>,
    /// Each earlier snapshot's new events, newest first, for as long as
    /// they are repeated.
    repeated: VecDeque<Vec<Event>>,
}

/// What a human has sent, applied one sequence per tick in order: once
/// the first is applied, tick by tick the next sequence's controls are, so
/// the tick an input is applied at is fixed by its sequence and a datagram
/// that lands late or never arrives costs one tick of the last controls,
/// never a skip or a slip of every later input (ADR 0019). The sequence
/// stops for a client that falls the tuned depth behind it, and jumps for
/// one the tuned depth ahead of it.
#[derive(Debug, Clone, Default)]
struct HumanInput {
    /// The controls applied last tick.
    applied: KartInput,
    /// The sequence applied last tick; 0 before the first.
    sequence: u32,
    /// The newest sequence received.
    newest: u32,
    /// Received and not yet applied, by sequence, every one newer than
    /// `sequence`.
    queued: BTreeMap<u32, KartInput>,
    /// Ticks since the last input arrived; `None` until the first does, so
    /// a client still loading the race is not taken for a lost one, and
    /// while a bot drives the kart.
    silent_ticks: Option<u32>,
}

impl HumanInput {
    /// Count a tick without an input; true the tick the silence reaches
    /// `limit`. A human not yet heard from is never silent.
    fn silent_for(&mut self, limit: u32) -> bool {
        let Some(ticks) = &mut self.silent_ticks else {
            return false;
        };
        *ticks += 1;
        *ticks >= limit
    }

    fn receive(&mut self, input: Input) {
        self.silent_ticks = Some(0);
        if input.sequence > self.sequence {
            self.queued.insert(input.sequence, KartInput::from(input));
            self.newest = self.newest.max(input.sequence);
        }
    }

    /// The controls for the coming tick.
    fn next(&mut self) -> KartInput {
        // The first input waits for a second, so the queue starts one input
        // ahead and a datagram that lands a little late is still in time.
        let waiting_to_start = self.sequence == 0 && self.queued.len() < INPUT_QUEUE_START_TICKS;
        let ahead_of_client = self.sequence >= self.newest + INPUT_QUEUE_MAX_TICKS;
        if waiting_to_start || ahead_of_client {
            return self.applied;
        }
        let sequence = (self.sequence + 1).max(self.newest.saturating_sub(INPUT_QUEUE_MAX_TICKS));
        while let Some(entry) = self.queued.first_entry() {
            if *entry.key() > sequence {
                break;
            }
            self.applied = entry.remove();
        }
        self.sequence = sequence;
        self.applied
    }
}

/// What one tick produced for the players.
#[derive(Debug, Clone, PartialEq)]
pub struct Ticked {
    /// What happened this tick, for the reliable stream.
    pub events: Vec<Event>,
    /// Present every `TICKS_PER_SNAPSHOT`th tick, for the datagram lane.
    pub snapshot: Option<Snapshot>,
    /// The countdown ended this tick: the room is Racing.
    pub go: bool,
    /// The race ended this tick: the results, in finishing order.
    pub results: Option<Vec<FinalStanding>>,
    /// The players heard nothing from for the tuned silence, whose karts a
    /// bot drives from this tick (ADR 0012).
    pub silenced: Vec<u8>,
}

impl Race {
    /// A race from `seed` on `track` with one human kart per slot in `grid`,
    /// in that order, and bots on the rest of the grid, at the top of its
    /// countdown, the bots driving at `bot_level`, the tyres as `tyres` says
    /// and over `laps`. Event ids count on from
    /// `first_event_id`, so a room's ids never repeat across its races.
    /// `grid` must fit the race's grid.
    pub fn new(
        seed: u64,
        track: Track,
        bot_level: BotLevel,
        tyres: Tyres,
        laps: u32,
        grid: Vec<u8>,
        first_event_id: u32,
    ) -> Self {
        assert!(grid.len() <= KARTS_PER_RACE, "more humans than karts");
        let bots = KARTS_PER_RACE - grid.len();
        Self {
            state: Some(
                State::with_tyres(seed, grid.len(), bots, track, bot_level, tyres).with_laps(laps),
            ),
            humans: vec![HumanInput::default(); grid.len()],
            grid,
            tick: 0,
            next_event_id: first_event_id,
            pending: Vec::new(),
            repeated: VecDeque::new(),
        }
    }

    /// The kart the player in `slot` drives, or `None` for a slot not on
    /// the grid.
    pub fn kart_of(&self, slot: u8) -> Option<usize> {
        self.grid.iter().position(|&seat| seat == slot)
    }

    /// Take the player's input for their kart. Only a sequence newer than
    /// the one held replaces it, so a datagram that arrives late or out of
    /// order changes nothing; sequence 0 is "nothing sent yet" and is never
    /// applied.
    pub fn receive(&mut self, slot: u8, input: Input) {
        let Some(kart) = self.kart_of(slot) else {
            return;
        };
        self.humans[kart].receive(input);
    }

    /// The player is gone or silent: a bot drives their kart from the
    /// next tick and what they sent is dropped, until [`Self::hand_back`].
    pub fn hand_to_bot(&mut self, slot: u8) {
        if let Some(kart) = self.kart_of(slot) {
            self.state_mut().hand_to_bot(kart);
            let human = &mut self.humans[kart];
            human.applied = KartInput::default();
            human.queued.clear();
            human.silent_ticks = None;
        }
    }

    /// The player is back: their inputs drive their kart again from the
    /// first one received now, whatever sequence it carries, and their
    /// silence counts from that first one, as at the start of the race.
    pub fn hand_back(&mut self, slot: u8) {
        if let Some(kart) = self.kart_of(slot) {
            self.state_mut().hand_back(kart);
            self.humans[kart] = HumanInput::default();
        }
    }

    fn state_mut(&mut self) -> &mut State {
        self.state
            .as_mut()
            .expect("state is only absent during a tick")
    }

    /// Ticks stepped so far: the race clock as the snapshots report it,
    /// for the log.
    pub fn ticks(&self) -> u32 {
        self.tick
    }

    /// The id the next event gets; a rematch's race continues from it.
    pub fn next_event_id(&self) -> u32 {
        self.next_event_id
    }

    /// Step one tick from the inputs held, and gather what the players are
    /// told about it. The caller stops ticking once `results` comes back: a
    /// later tick would report the results again.
    pub fn tick(&mut self) -> Ticked {
        let silenced = self.silence();
        let state = self
            .state
            .take()
            .expect("state is only absent during a tick");
        let was_counting_down = matches!(state.phase(), Phase::Countdown { .. });
        let inputs: Vec<KartInput> = self.humans.iter_mut().map(HumanInput::next).collect();
        let state = sim::step(state, &inputs, TICK_S);
        self.tick += 1;

        let events: Vec<Event> = state
            .events()
            .iter()
            .map(|&event| {
                let id = self.next_event_id;
                self.next_event_id += 1;
                Event {
                    id,
                    kind: EventKind::from(event),
                }
            })
            .collect();
        self.pending.extend(events.iter().copied());
        let snapshot = self
            .tick
            .is_multiple_of(TICKS_PER_SNAPSHOT)
            .then(|| self.snapshot(&state));
        let go = was_counting_down && matches!(state.phase(), Phase::Racing { .. });
        let results = matches!(state.phase(), Phase::Finished).then(|| results(&state));
        self.state = Some(state);
        Ticked {
            events,
            snapshot,
            go,
            results,
            silenced,
        }
    }

    /// Count a tick of silence against every human heard from before, and
    /// hand to a bot each one silent for the tuned time.
    fn silence(&mut self) -> Vec<u8> {
        let limit = (CONNECTION_SILENCE_S * TICKS_PER_SECOND as f32).round() as u32;
        let mut silenced = Vec::new();
        for kart in 0..self.humans.len() {
            if self.humans[kart].silent_for(limit) {
                let slot = self.grid[kart];
                self.hand_to_bot(slot);
                silenced.push(slot);
            }
        }
        silenced
    }

    /// The state at this tick with every event since the last snapshot and
    /// those of the snapshots still repeating them.
    fn snapshot(&mut self, state: &State) -> Snapshot {
        let events: Vec<Event> = self
            .pending
            .iter()
            .chain(self.repeated.iter().flatten())
            .copied()
            .collect();
        self.repeated.push_front(std::mem::take(&mut self.pending));
        self.repeated.truncate(SNAPSHOT_EVENT_REPEATS);
        let karts = state.kart_readouts();
        let hazards = state.hazards().iter().map(HazardSnapshot::from).collect();
        let projectiles = state
            .projectiles()
            .iter()
            .map(ProjectileSnapshot::from)
            .collect();
        let mut acknowledged: Vec<u32> = self.humans.iter().map(|human| human.sequence).collect();
        acknowledged.resize(karts.len(), 0);
        Snapshot {
            tick: self.tick,
            acknowledged,
            karts,
            events,
            hazards,
            projectiles,
            coins_present: state.coins_present(),
        }
    }
}

/// The standings as the results: every kart, in finishing order.
fn results(state: &State) -> Vec<FinalStanding> {
    let mut results: Vec<FinalStanding> = state
        .standings()
        .iter()
        .enumerate()
        .map(|(kart, standing)| FinalStanding {
            kart: kart as u8,
            position: standing.position as u8,
            finish_time_s: standing.finish_time_s,
            best_lap_s: standing.best_lap_s,
            stops: standing.stops as u8,
            best_stop_s: standing.best_stop_s,
        })
        .collect();
    results.sort_by_key(|standing| standing.position);
    results
}
