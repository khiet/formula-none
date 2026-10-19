//! One room's race on the server, the only place its outcome is decided
//! (ADR 0009): the sim stepped at the tick rate from the latest input each
//! human has sent, the snapshots and events the players are told, and the
//! results once it is over. Pure bookkeeping with no clock and no channel,
//! so the room task in `rooms` decides when a tick happens and where its
//! output goes, and this module decides what a tick produces.

use std::collections::VecDeque;

use protocol::{
    DriftDirection, DriftSnapshot, Event, EventKind, FinalStanding, Input, KartSnapshot, Snapshot,
};
use sim::tuning::{
    KARTS_PER_RACE, SNAPSHOTS_PER_SECOND, SNAPSHOT_EVENT_REPEATS, TICKS_PER_SECOND,
};
use sim::{KartInput, Phase, RaceEvent, State, Track};

/// Seconds of one tick: the sim's `dt` and the budget a tick's work must fit
/// in for the server to keep up (ADR 0010).
pub const TICK_S: f32 = 1.0 / TICKS_PER_SECOND as f32;
/// Ticks between snapshots.
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

/// The newest input a human has sent, held until a newer sequence arrives
/// so a lost or late datagram leaves the kart doing what it was told last.
#[derive(Debug, Clone, Copy, Default)]
struct HumanInput {
    latest: KartInput,
    sequence: u32,
}

/// What one tick produced for the players.
#[derive(Debug, Clone, PartialEq)]
pub struct Ticked {
    /// What happened this tick, for the reliable stream.
    pub events: Vec<Event>,
    /// Every snapshot-rate tick, for the datagram lane.
    pub snapshot: Option<Snapshot>,
    /// The countdown ended this tick: the room is Racing.
    pub go: bool,
    /// The race ended this tick: the results, in finishing order.
    pub results: Option<Vec<FinalStanding>>,
}

impl Race {
    /// A race from `seed` on `track` with one human kart per slot in `grid`,
    /// in that order, and bots on the rest of the grid, at the top of its
    /// countdown. Event ids count on from `first_event_id`, so a room's
    /// ids never repeat across its races. `grid` must fit the race's grid.
    pub fn new(seed: u64, track: Track, grid: Vec<u8>, first_event_id: u32) -> Self {
        assert!(grid.len() <= KARTS_PER_RACE, "more humans than karts");
        let bots = KARTS_PER_RACE - grid.len();
        Self {
            state: Some(State::with_bots(seed, grid.len(), bots, track)),
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
        let human = &mut self.humans[kart];
        if input.sequence > human.sequence {
            human.sequence = input.sequence;
            human.latest = kart_input(input);
        }
    }

    /// The player is gone: their kart holds no controls until they are back.
    pub fn release(&mut self, slot: u8) {
        if let Some(kart) = self.kart_of(slot) {
            self.humans[kart].latest = KartInput::default();
        }
    }

    /// The id the next event gets; a rematch's race continues from it.
    pub fn next_event_id(&self) -> u32 {
        self.next_event_id
    }

    /// Step one tick from the inputs held, and gather what the players are
    /// told about it. A finished race changes nothing and produces nothing.
    pub fn tick(&mut self) -> Ticked {
        let state = self
            .state
            .take()
            .expect("state is only absent during a tick");
        let was_counting_down = matches!(state.phase(), Phase::Countdown { .. });
        let inputs: Vec<KartInput> = self.humans.iter().map(|human| human.latest).collect();
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
                    kind: event_kind(event),
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
        }
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
        self.repeated
            .push_front(std::mem::take(&mut self.pending));
        self.repeated.truncate(SNAPSHOT_EVENT_REPEATS);
        let standings = state.standings();
        let karts = state
            .kart_transforms()
            .iter()
            .zip(&standings)
            .map(|(transform, standing)| KartSnapshot {
                position: transform.position,
                rotation: transform.rotation,
                velocity: transform.velocity,
                drift: transform.drift.map(|drift| DriftSnapshot {
                    direction: match drift.direction {
                        sim::DriftDirection::Left => DriftDirection::Left,
                        sim::DriftDirection::Right => DriftDirection::Right,
                    },
                    charge_s: drift.charge_s,
                }),
                race_position: standing.position as u8,
                lap: standing.lap() as u8,
                checkpoint: standing.checkpoint as u8,
                spline_progress: standing.spline_progress,
                wrong_way: standing.wrong_way,
                held_item: None,
                spin_out_s: None,
            })
            .collect();
        let mut acknowledged: Vec<u32> = self.humans.iter().map(|human| human.sequence).collect();
        acknowledged.resize(standings.len(), 0);
        Snapshot {
            tick: self.tick,
            acknowledged,
            karts,
            events,
        }
    }
}

/// The sim's input for a wire input: each byte back to the unit range.
fn kart_input(input: Input) -> KartInput {
    KartInput {
        steer: (f32::from(input.steer) / f32::from(i8::MAX)).clamp(-1.0, 1.0),
        throttle: f32::from(input.throttle) / f32::from(u8::MAX),
        brake: f32::from(input.brake) / f32::from(u8::MAX),
        hop: input.hop,
        use_item: input.use_item,
        hold_item: input.hold_item,
    }
}

fn event_kind(event: RaceEvent) -> EventKind {
    match event {
        RaceEvent::LapCompleted {
            kart,
            lap,
            lap_time_s,
        } => EventKind::LapCompleted {
            kart: kart as u8,
            lap: lap as u8,
            lap_time_s,
        },
        RaceEvent::Finished {
            kart,
            position,
            finish_time_s,
        } => EventKind::Finished {
            kart: kart as u8,
            position: position as u8,
            finish_time_s,
        },
        RaceEvent::Respawned { kart } => EventKind::Respawned { kart: kart as u8 },
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
        })
        .collect();
    results.sort_by_key(|standing| standing.position);
    results
}
