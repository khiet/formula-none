//! The player's own kart, predicted in the browser so it answers the
//! controls the tick they change (ADR 0009). The prediction is this crate's
//! `step` on a race holding only that kart on the static track, so it runs
//! the server's rules and nothing else. The other karts are the snapshots':
//! each tick they stand where the client draws them, as kinematic bodies
//! the own kart meets and cannot push, so it never drives through one.
//! Kart-to-kart contact is the server's to resolve; a kart drawn 100 ms
//! behind can leave a small correction after a touch.
//!
//! Two things the prediction never runs. The item roll: the slot is the
//! snapshots' word, so a fire is predicted only once the server has said
//! what the kart holds. And the hazards: a hazard the predicted kart fires
//! is gone from this race the same tick, so the kart never hits one here;
//! a hit is the server's, and reaches the kart as the spin-out timer in a
//! snapshot, which holds it still through the replay.
//!
//! One thing it runs and then defers on: the rocket start. The prediction
//! reads the hold as the server will, so the sparks and the boost answer
//! the press at once; but a press at a window's edge can land a tick off
//! on the server, so from Go the snapshots' verdict is adopted, and the
//! boost or the stall it gives is set as far along as Go is behind.
//!
//! Each tick the client steps the prediction with the input it sends. Each
//! snapshot is held against what the prediction said of the kart at the
//! snapshot's tick: when the two agree nothing happens, and when they do not
//! the prediction rewinds to the snapshot and replays every input the
//! server has yet to apply. The jump that leaves in the kart is counted as
//! a correction and blended out of the drawn kart over the tuned ticks.
//!
//! The server applies one sequence per tick, in order (ADR 0019), so
//! once a snapshot of tick T acknowledges sequence A, sequence A + n is
//! applied at tick T + n: the prediction lays its inputs out on the
//! server's ticks by that lead and replays them there. The lead moves only
//! when the client stalls or the server drops inputs to catch up.

use std::collections::VecDeque;

use rapier3d::prelude::*;

use crate::tuning::{
    COUNTDOWN_S, KART_HALF_LENGTH_M, RECONCILIATION_BLEND_TICKS, RECONCILIATION_TOLERANCE_M, TICK_S,
};
use crate::{
    apply_start, clock_after, drift_slip, kart_collider, step, Drift, Item, Kart, KartInput,
    KartTransform, Phase, Standing, Start, State, Surface, Track,
};

/// What a snapshot says of one kart, in the sim's terms. Metres, seconds and
/// metres per second; Y up; the rotation is a unit quaternion [x, y, z, w].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapshotKart {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub drift: Option<Drift>,
    /// The item in the kart's slot, which only the server rolls.
    pub held_item: Option<Item>,
    /// Seconds of spin-out left, while the kart is spun out; only the
    /// server decides a hit.
    pub spin_out_s: Option<f32>,
    /// The kart's rocket start as the server has it: from Go, its verdict.
    pub start: Start,
    /// Seconds of stall left, while the kart is stalled at Go.
    pub stall_s: Option<f32>,
    /// The lap the kart is on, from 1, as [`Standing::lap`] counts it.
    pub lap: u32,
    /// The last checkpoint crossed in order; 0 is the start line.
    pub checkpoint: usize,
    pub spline_progress: f32,
}

/// One player's kart as their client predicts it.
pub struct Prediction {
    /// The kart's grid slot: its entry among the karts `tick` is given is
    /// skipped.
    kart: usize,
    /// A kinematic body for each other kart in grid order, once the client
    /// has drawn the other karts.
    others: Vec<RigidBodyHandle>,
    /// A race with the one kart, on its own spawn slot. Its phase is the
    /// race clock at `tick` and never Finished: the server's race ends, not
    /// this one, and a finished kart drives on until it does. `None` only
    /// inside a step, since `step` consumes the state.
    state: Option<State>,
    /// The server tick the predicted kart stands at.
    tick: u32,
    /// The kart at the newest snapshot it was reconciled with, then one
    /// entry per input since, one tick apart.
    entries: VecDeque<Entry>,
    /// The race clock at `clock_tick`, the newest snapshot's tick; it only
    /// runs forward, as snapshots do.
    clock: Phase,
    clock_tick: u32,
    /// The server tick less the sequence applied at it, which the entries
    /// were last laid out by; `None` before an acknowledgement.
    lead: Option<u32>,
    corrections: u32,
    /// Racing ticks predicted, which the corrections per second are read
    /// over; the countdown holds the kart still, so it corrects nothing.
    ticks: u32,
    blend: Blend,
}

/// An input the prediction applied and the kart it left.
struct Entry {
    sequence: u32,
    /// The server tick the input is predicted to be applied at.
    tick: u32,
    input: KartInput,
    /// Where the other karts stood at the end of the tick.
    others: Vec<Placed>,
    after: KartRecord,
}

/// Where a kart stands and which way it faces.
#[derive(Clone, Copy)]
struct Placed {
    translation: Vec3,
    rotation: Rotation,
}

/// Everything about a kart that a later tick reads, so a rewind can put it
/// back: the sim's own fields and its body's motion.
#[derive(Clone)]
struct KartRecord {
    kart: Kart,
    translation: Vec3,
    rotation: Rotation,
    linvel: Vec3,
    angvel: Vec3,
}

/// What is left of the last correction to blend out of the drawn kart: the
/// drawn kart is offset from the predicted one by this much, scaled by the
/// ticks left over the tuned ticks.
#[derive(Clone, Copy)]
struct Blend {
    position: Vec3,
    rotation: Rotation,
    ticks_left: u32,
}

impl Blend {
    const NONE: Self = Self {
        position: Vec3::ZERO,
        rotation: Rotation::IDENTITY,
        ticks_left: 0,
    };

    fn share(&self) -> f32 {
        self.ticks_left as f32 / RECONCILIATION_BLEND_TICKS as f32
    }
}

impl Prediction {
    /// The kart on grid slot `kart` of a race from `seed` on `track`, at the
    /// top of its countdown. `kart` must be a slot of the track's grid.
    pub fn new(seed: u64, kart: usize, track: Track) -> Self {
        assert!(
            kart < track.spawn_grid().len(),
            "kart {kart} but the track has {} spawn slots",
            track.spawn_grid().len()
        );
        let mut state = State::on_spawn_slots(seed, 1, kart..kart + 1, track);
        // A box's roll is the server's: its RNG serves every kart's pickups
        // in the order they happen, which this race of one cannot follow.
        // So the boxes are never here to pick up, and the slot holds only
        // what the snapshots say, which a press then fires as the server
        // fires it.
        state.item_box_respawn_s = vec![f32::INFINITY; state.item_box_respawn_s.len()];
        let spawn = Entry {
            sequence: 0,
            tick: 0,
            input: KartInput::default(),
            others: Vec::new(),
            after: state.record(),
        };
        Self {
            kart,
            others: Vec::new(),
            state: Some(state),
            tick: 0,
            entries: VecDeque::from([spawn]),
            clock: Phase::Countdown {
                remaining_s: COUNTDOWN_S,
            },
            clock_tick: 0,
            lead: None,
            corrections: 0,
            ticks: 0,
            blend: Blend::NONE,
        }
    }

    pub fn track(&self) -> &Track {
        self.state().track()
    }

    /// Step the kart one tick with the input the client sends the server
    /// this tick under `sequence`, against `karts`, every kart in grid
    /// order as the client draws them this tick; this kart's own entry is
    /// not read, and before the client draws any it is empty. `input` must
    /// be what the server will read, the wire's bytes back in the unit
    /// range, and the sequences must count up by one from 1.
    pub fn tick(&mut self, sequence: u32, input: KartInput, karts: &[KartTransform]) {
        let others: Vec<Placed> = karts
            .iter()
            .enumerate()
            .filter(|&(slot, _)| slot != self.kart)
            .map(|(_, kart)| Placed {
                translation: Vec3::from(kart.position),
                rotation: Rotation::from_array(kart.rotation).normalize(),
            })
            .collect();
        self.advance(input, &others);
        self.entries.push_back(Entry {
            sequence,
            tick: self.tick,
            input,
            others,
            after: self.state().record(),
        });
        if let Phase::Racing { .. } = self.state().phase {
            self.ticks += 1;
        }
        self.blend.ticks_left = self.blend.ticks_left.saturating_sub(1);
    }

    /// Hold the snapshot of `snapshot_tick`, which acknowledged `acknowledged`
    /// and said `kart` of this kart, against the prediction. Returns how far
    /// the predicted kart moved for it, in metres: 0 when the snapshot
    /// agreed, or is older than one already held, or acknowledges nothing.
    pub fn reconcile(&mut self, snapshot_tick: u32, acknowledged: u32, kart: &SnapshotKart) -> f32 {
        if snapshot_tick <= self.clock_tick {
            return 0.0;
        }
        while self.clock_tick < snapshot_tick {
            self.clock = clock_after(self.clock, TICK_S);
            self.clock_tick += 1;
        }
        // With nothing acknowledged there is no lead to lay the inputs out by.
        if acknowledged == 0 {
            return 0.0;
        }
        let lead = snapshot_tick.saturating_sub(acknowledged);
        let relaid = self.lead != Some(lead);
        if relaid {
            self.lead = Some(lead);
            for entry in self.entries.iter_mut().skip(1) {
                entry.tick = entry.sequence + lead;
            }
        }
        // The entry at the snapshot's tick, or else the last before it.
        let at = self
            .entries
            .iter()
            .rposition(|entry| entry.tick <= snapshot_tick)
            .unwrap_or(0);
        self.entries.drain(..at);
        let anchor = &self.entries[0];
        let racing = matches!(self.clock, Phase::Racing { .. });
        if !relaid && anchor.tick == snapshot_tick && agrees(&anchor.after, kart, racing) {
            return 0.0;
        }

        let drawn = self.kart_transform();
        let predicted = self.state().kart_transforms()[0].position;
        let mut record = anchor.after.clone();
        let clock = self.clock;
        record.take(kart, clock);
        let anchor_others = self.entries[0].others.clone();
        self.place_others(&anchor_others, true);
        let state = self.state_mut();
        state.restore(&record);
        state.phase = clock;
        self.tick = snapshot_tick;
        let anchor = &mut self.entries[0];
        anchor.tick = snapshot_tick;
        anchor.after = record;
        for index in 1..self.entries.len() {
            let input = self.entries[index].input;
            let others = std::mem::take(&mut self.entries[index].others);
            self.advance(input, &others);
            self.entries[index].others = others;
            let after = self.state().record();
            let entry = &mut self.entries[index];
            entry.tick = self.tick;
            entry.after = after;
        }

        let after = self.state().kart_transforms()[0];
        let moved = Vec3::from(predicted).distance(Vec3::from(after.position));
        self.blend = Blend {
            position: Vec3::from(drawn.position) - Vec3::from(after.position),
            rotation: Rotation::from_array(drawn.rotation)
                * Rotation::from_array(after.rotation).inverse(),
            ticks_left: RECONCILIATION_BLEND_TICKS,
        };
        if moved > RECONCILIATION_TOLERANCE_M {
            self.corrections += 1;
        }
        moved
    }

    /// The kart as it is drawn: where the prediction has it, less what is
    /// left to blend of the last correction.
    pub fn kart_transform(&self) -> KartTransform {
        let mut transform = self.state().kart_transforms()[0];
        let share = self.blend.share();
        if share > 0.0 {
            let position = Vec3::from(transform.position) + self.blend.position * share;
            let rotation = Rotation::IDENTITY.lerp(self.blend.rotation, share)
                * Rotation::from_array(transform.rotation);
            transform.position = position.to_array();
            transform.rotation = rotation.normalize().to_array();
        }
        transform
    }

    /// The surface under the predicted kart.
    pub fn kart_surface(&self) -> Surface {
        self.state().kart_surface(0)
    }

    /// What lies under `point` on the track; karts are never an answer.
    pub fn surface_at(&self, point: [f32; 3]) -> Surface {
        self.state().surface_at(point)
    }

    /// The predicted kart's standing; its race position counts only itself.
    pub fn standing(&self) -> Standing {
        self.state().standings()[0]
    }

    /// The race clock at the predicted kart's tick: the countdown the
    /// player's inputs will meet on the server.
    pub fn phase(&self) -> Phase {
        self.state().phase
    }

    /// Snapshots that moved the predicted kart further than rounding.
    pub fn corrections(&self) -> u32 {
        self.corrections
    }

    /// The corrections over the seconds raced so far; 0 before Go.
    pub fn corrections_per_second(&self) -> f32 {
        if self.ticks == 0 {
            return 0.0;
        }
        self.corrections as f32 / (self.ticks as f32 * TICK_S)
    }

    /// One tick of the sim against the other karts moving to `others`, with
    /// the race clock kept the server's.
    fn advance(&mut self, input: KartInput, others: &[Placed]) {
        self.place_others(others, false);
        let state = self
            .state
            .take()
            .expect("state is only absent during a step");
        let clock = clock_after(state.phase, TICK_S);
        let mut state = step(state, &[input], TICK_S);
        state.phase = clock;
        state.hazards.clear();
        // A projectile the predicted kart fires has no kart ahead in this
        // race of one and is wasted; cleared all the same, so the hit
        // stays the server's whatever a targetless fire comes to do.
        state.projectiles.clear();
        self.state = Some(state);
        self.tick += 1;
    }

    /// Stand the other karts at `others`: there and then when `teleport`,
    /// else by the end of the next step, so the own kart meets them moving.
    /// A kart seen for the first time is put there.
    fn place_others(&mut self, others: &[Placed], teleport: bool) {
        let state = self
            .state
            .as_mut()
            .expect("state is only absent during a step");
        let physics = &mut state.physics;
        while self.others.len() < others.len() {
            let placed = others[self.others.len()];
            let body = physics.bodies.insert(
                RigidBodyBuilder::kinematic_position_based()
                    .translation(placed.translation)
                    .rotation(placed.rotation.to_scaled_axis())
                    .build(),
            );
            physics
                .colliders
                .insert_with_parent(kart_collider(), body, &mut physics.bodies);
            self.others.push(body);
        }
        for (&body, placed) in self.others.iter().zip(others) {
            let body = &mut physics.bodies[body];
            if teleport {
                body.set_translation(placed.translation, true);
                body.set_rotation(placed.rotation, true);
            } else {
                body.set_next_kinematic_translation(placed.translation);
                body.set_next_kinematic_rotation(placed.rotation);
            }
        }
    }

    fn state(&self) -> &State {
        self.state
            .as_ref()
            .expect("state is only absent during a step")
    }

    fn state_mut(&mut self) -> &mut State {
        self.state
            .as_mut()
            .expect("state is only absent during a step")
    }
}

/// Whether the snapshot puts the kart where the record does, to the tuned
/// tolerance: its position, its nose, the tick of travel its velocity
/// makes, its drift, its item, its spin-out, its stall, its place in the
/// lap and, once `racing`, its rocket start's verdict; during the countdown
/// the start is not compared, since `take` adopts it only from Go.
fn agrees(record: &KartRecord, kart: &SnapshotKart, racing: bool) -> bool {
    let nose = |rotation: Rotation| rotation * Vec3::NEG_Z * KART_HALF_LENGTH_M;
    let near = |a: Vec3, b: Vec3| a.distance(b) <= RECONCILIATION_TOLERANCE_M;
    let drift = match (record.kart.drift, kart.drift) {
        (None, None) => true,
        (Some(ours), Some(theirs)) => {
            ours.direction == theirs.direction
                && (ours.charge_s - theirs.charge_s).abs() < TICK_S / 2.0
        }
        _ => false,
    };
    near(record.translation, Vec3::from(kart.position))
        && near(
            nose(record.rotation),
            nose(Rotation::from_array(kart.rotation)),
        )
        && near(record.linvel * TICK_S, Vec3::from(kart.velocity) * TICK_S)
        && drift
        && record.kart.held_item == kart.held_item
        && (record.kart.spin_out_left_s.max(0.0) - kart.spin_out_s.unwrap_or(0.0)).abs()
            < TICK_S / 2.0
        && (record.kart.stall_left_s.max(0.0) - kart.stall_s.unwrap_or(0.0)).abs() < TICK_S / 2.0
        && (!racing || record.kart.start == kart.start)
        && lap(&record.kart) == kart.lap
        && record.kart.checkpoint == kart.checkpoint
}

/// The lap a kart is on, as [`Standing::lap`] counts it.
fn lap(kart: &Kart) -> u32 {
    (kart.laps_completed + 1).min(crate::LAPS_PER_RACE)
}

impl KartRecord {
    /// The snapshot's word on everything it carries, at `clock`, the race
    /// clock at the snapshot's tick. What it does not carry stays the
    /// prediction's, which ran the same rules on the same inputs; the speed
    /// the handling holds is read back from the velocity, as the sim reads
    /// it after a wall. From Go, a rocket start verdict other than the
    /// prediction's replaces the boost or stall the prediction gave the
    /// kart at Go with the server's, run down by the ticks since.
    fn take(&mut self, snapshot: &SnapshotKart, clock: Phase) {
        let rotation = Rotation::from_array(snapshot.rotation).normalize();
        let velocity = Vec3::from(snapshot.velocity);
        self.translation = Vec3::from(snapshot.position);
        self.rotation = rotation;
        self.linvel = velocity;
        let kart = &mut self.kart;
        kart.drift = snapshot.drift;
        let slip = kart.drift.map_or(0.0, |drift| drift_slip(drift.direction));
        kart.travel = rotation * Rotation::from_rotation_y(slip) * Vec3::NEG_Z;
        // A snapshot does not carry the bump a touch left, so the
        // prediction's own stands, from the touch it saw against the
        // kinematic stand-in; the speed is what the velocity holds along
        // the travel once that bump is taken off, as the sim built it.
        kart.speed = kart.travel.dot(velocity - kart.bump);
        kart.held_item = snapshot.held_item;
        kart.spin_out_left_s = snapshot.spin_out_s.unwrap_or(0.0);
        if let Phase::Racing { elapsed_s, .. } = clock {
            if kart.start != snapshot.start {
                kart.start = snapshot.start;
                kart.boost_left_s = 0.0;
                apply_start(kart, snapshot.start);
                // The boost's clock, run down a tick at a time as the
                // server's was, so the two spend it on the same tick.
                let ticks_since_go = (elapsed_s / TICK_S).round() as u32;
                for _ in 0..ticks_since_go {
                    if kart.boost_left_s > 0.0 {
                        kart.boost_left_s -= TICK_S;
                    }
                }
            }
        }
        kart.stall_left_s = snapshot.stall_s.unwrap_or(0.0);
        if lap(kart) != snapshot.lap {
            kart.laps_completed = snapshot.lap.saturating_sub(1);
        }
        kart.checkpoint = snapshot.checkpoint;
        kart.spline_progress = snapshot.spline_progress;
    }
}

impl State {
    /// The one kart of a prediction, as a record.
    fn record(&self) -> KartRecord {
        let kart = &self.karts[0];
        let body = &self.physics.bodies[kart.body];
        KartRecord {
            kart: kart.clone(),
            translation: body.translation(),
            rotation: *body.rotation(),
            linvel: body.linvel(),
            angvel: body.angvel(),
        }
    }

    /// Put the one kart of a prediction back as `record` has it.
    fn restore(&mut self, record: &KartRecord) {
        let kart = &mut self.karts[0];
        *kart = record.kart.clone();
        let body = &mut self.physics.bodies[kart.body];
        body.set_translation(record.translation, true);
        body.set_rotation(record.rotation, true);
        body.set_linvel(record.linvel, true);
        body.set_angvel(record.angvel, true);
        body.lock_translations(kart.spun_out(), true);
    }
}
