//! The player's own kart, predicted in the browser so it answers the
//! controls the tick they change (ADR 0009). The prediction is this crate's
//! `step` on a race holding only that kart on the static track, so it runs
//! the server's rules and nothing else. The other karts are the snapshots':
//! each tick they stand where the client draws them, as kinematic bodies
//! the own kart meets and cannot push, so it never drives through one.
//! Kart-to-kart contact is the server's to resolve; a kart drawn 100 ms
//! behind can leave a small correction after a touch.
//!
//! The runaway wheel it runs in full: it is the race clock's and the
//! seed's, which the prediction shares with the server, so a wheel hit is
//! predicted the tick it happens, and the snapshots' spin-out timer then
//! agrees with it.
//!
//! The tyres it wears as the server does, since that follows from the
//! kart's own inputs; wear from other karts and hits is the server's, so
//! the snapshots' tyre life stands. A snapshot within the tuned tolerance
//! is no reason to rewind (ADR 0080).
//!
//! Four things the prediction never runs. The item roll: the slot is the
//! snapshots' word, so a fire is predicted only once the server has said
//! what the kart holds. The coin pickups: a coin another kart took first
//! is the server's to know, so the count is the snapshots' word too, and a
//! spin-out's loss predicted here gives way to it. The hazards: a
//! hazard the predicted kart fires is gone from this race the same tick, so
//! the kart never hits one here; a hit is the server's, and reaches the
//! kart as the spin-out timer in a snapshot, which holds it still through
//! the replay. And the wake (ADR 0077): the other karts stand here 100 ms
//! behind, so a verdict at the wake's edge would differ from the server's,
//! and a tow fired or withheld by mistake is a correction of metres. The
//! slipstream is the snapshots' word, carried on between them as the
//! server would carry it while the kart stays where the snapshot had it:
//! a charge counts on, a full one keeps the tow armed, a tow out of the
//! wake runs down. A snapshot that says otherwise is a rewind that adopts
//! it; a wake's entry therefore reaches the own kart about one round trip
//! late, which no button press is waiting on, and the tow then takes on
//! the server's tick.
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
    COUNTDOWN_S, KART_HALF_LENGTH_M, RACE_OUTRO_S, RECONCILIATION_BLEND_TICKS,
    RECONCILIATION_TOLERANCE_M, RECONCILIATION_TYRE_LIFE_TOLERANCE_PERCENT, TICK_S,
};
use crate::BotLevel;
use crate::{
    bot, clock_after, kart_collider, kart_groups, step, ItemSlot, Kart, KartInput, KartReadout,
    Phase, Slipstream, Standing, State, Surface, Track, Tyres, Wheel,
};

/// The phase a prediction holds once the server's race has ended: the top
/// of the outro, which it never runs down.
const ENDED: Phase = Phase::Outro {
    remaining_s: RACE_OUTRO_S,
};

/// One player's kart as their client predicts it.
pub struct Prediction {
    /// The kart's grid slot: its entry among the karts `tick` is given is
    /// skipped.
    kart: usize,
    /// A kinematic body for each other kart in grid order, once the client
    /// has drawn the other karts.
    others: Vec<RigidBodyHandle>,
    /// A race with the one kart, on its own spawn slot. Its phase is the
    /// race clock at `tick` until the client is told the server's race has
    /// ended, then the outro, held there: never Finished, since the
    /// server's race ends, not this one. A finished kart drives on under
    /// its bot until it does.
    /// `None` only inside a step, since `step` consumes the state.
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
    /// Counted only while the player drives the kart: past the line the
    /// bot here sees no other kart, so it steers apart from the server's.
    corrections: u32,
    /// Racing ticks predicted before the finish, which the corrections per
    /// second are read over; the countdown holds the kart still, so it
    /// corrects nothing.
    ticks: u32,
    blend: Blend,
    /// Whether the client has been told the server's race has ended.
    ended: bool,
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

/// Where a kart stands, which way it faces and whether it is on a star's
/// run, which its stand-in's collider takes from it so the own kart passes
/// through it as the server has it.
#[derive(Clone, Copy)]
struct Placed {
    translation: Vec3,
    rotation: Rotation,
    starring: bool,
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
    /// top of its countdown, driven by a bot at `level` once it has
    /// finished, as the server's is (ADR 0078). `kart` must be a slot of
    /// the track's grid.
    pub fn new(seed: u64, kart: usize, track: Track, level: BotLevel) -> Self {
        Self::with_tyres(seed, kart, track, level, Tyres::Off)
    }

    /// As [`Prediction::new`], with the creator's pick of `tyres`, so the
    /// kart wears its own tyres from its own inputs as the server's does.
    pub fn with_tyres(seed: u64, kart: usize, track: Track, level: BotLevel, tyres: Tyres) -> Self {
        assert!(
            kart < track.spawn_grid().len(),
            "kart {kart} but the track has {} spawn slots",
            track.spawn_grid().len()
        );
        let mut state = State::on_spawn_slots(seed, 1, kart..kart + 1, track, level, tyres);
        // A box's roll is the server's: its RNG serves every kart's pickups
        // in the order they happen, which this race of one cannot follow.
        // So the boxes are never here to pick up, and the slot holds only
        // what the snapshots say, which a press then fires as the server
        // fires it.
        state.item_box_respawn_s = vec![f32::INFINITY; state.item_box_respawn_s.len()];
        state.coin_respawn_s = vec![f32::INFINITY; state.coin_respawn_s.len()];
        // The wake is the server's to judge: the other karts stand here
        // where the client draws them, 100 ms behind, so a verdict at the
        // wake's edge would differ from the server's. The slipstream is
        // the snapshots' word, carried on between them.
        state.wakes_judged = false;
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
            ended: false,
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
    pub fn tick(&mut self, sequence: u32, input: KartInput, karts: &[KartReadout]) {
        let others: Vec<Placed> = karts
            .iter()
            .enumerate()
            .filter(|&(slot, _)| slot != self.kart)
            .map(|(_, kart)| Placed {
                translation: Vec3::from(kart.position),
                rotation: Rotation::from_array(kart.rotation).normalize(),
                starring: kart.star_s.is_some(),
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
            self.ticks += u32::from(!self.finished());
        }
        self.blend.ticks_left = self.blend.ticks_left.saturating_sub(1);
    }

    /// Hold the snapshot of `snapshot_tick`, which acknowledged `acknowledged`
    /// and read `kart` for this kart, against the prediction. Returns how far
    /// the predicted kart moved for it, in metres: 0 when the snapshot
    /// agreed, or is older than one already held, or acknowledges nothing.
    pub fn reconcile(&mut self, snapshot_tick: u32, acknowledged: u32, kart: &KartReadout) -> f32 {
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
        let racing = matches!(self.clock, Phase::Racing { .. }) && !self.ended;
        if !relaid && anchor.tick == snapshot_tick && agrees(&anchor.after, kart, racing) {
            return 0.0;
        }

        let drawn = self.kart_readout();
        let predicted = self.state().kart_readouts()[0].position;
        let mut record = anchor.after.clone();
        let clock = self.clock;
        record.take(kart, clock);
        let anchor_others = self.entries[0].others.clone();
        self.place_others(&anchor_others, true);
        let phase = if self.ended { ENDED } else { clock };
        let state = self.state_mut();
        state.restore(&record);
        state.phase = phase;
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

        let after = self.state().kart_readouts()[0];
        let moved = Vec3::from(predicted).distance(Vec3::from(after.position));
        self.blend = Blend {
            position: Vec3::from(drawn.position) - Vec3::from(after.position),
            rotation: Rotation::from_array(drawn.rotation)
                * Rotation::from_array(after.rotation).inverse(),
            ticks_left: RECONCILIATION_BLEND_TICKS,
        };
        if moved > RECONCILIATION_TOLERANCE_M && !self.finished() && !self.ended {
            self.corrections += 1;
        }
        moved
    }

    /// The kart as it is drawn: where the prediction has it, less what is
    /// left to blend of the last correction. Its race position, like
    /// [`Prediction::standing`]'s, counts only itself.
    pub fn kart_readout(&self) -> KartReadout {
        let mut readout = self.state().kart_readouts()[0];
        let share = self.blend.share();
        if share > 0.0 {
            let position = Vec3::from(readout.position) + self.blend.position * share;
            let rotation = Rotation::IDENTITY.lerp(self.blend.rotation, share)
                * Rotation::from_array(readout.rotation);
            readout.position = position.to_array();
            readout.rotation = rotation.normalize().to_array();
        }
        readout
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

    /// The runaway wheel at each of the track's crossings at the predicted
    /// kart's tick: the wheel that hits it here, where the server's will.
    pub fn wheels(&self) -> Vec<Wheel> {
        self.state().wheels()
    }

    /// The server's race has ended: from now on the prediction runs the
    /// outro the server does, a bot driving the kart, no items and no wheel,
    /// so nothing hits the kart here that the server's outro would not.
    pub fn end_race(&mut self) {
        if self.ended {
            return;
        }
        self.ended = true;
        let state = self.state_mut();
        state.end_race();
        state.phase = ENDED;
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
        let mut state = state;
        if self.ended {
            // A rewind puts back a kart from before the end, without its bot.
            state.karts[0].bot.get_or_insert_with(bot::Memory::default);
            state.phase = ENDED;
        }
        let clock = clock_after(state.phase, TICK_S);
        let mut state = step(state, &[input], TICK_S);
        state.phase = if self.ended { ENDED } else { clock };
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
            for &collider in body.colliders() {
                physics.colliders[collider].set_collision_groups(kart_groups(placed.starring));
            }
            if teleport {
                body.set_translation(placed.translation, true);
                body.set_rotation(placed.rotation, true);
            } else {
                body.set_next_kinematic_translation(placed.translation);
                body.set_next_kinematic_rotation(placed.rotation);
            }
        }
    }

    /// Whether the predicted kart is past the line on its last lap.
    fn finished(&self) -> bool {
        self.state().karts[0].finished()
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

/// Whether the snapshot's item slot is the record's: the same item, and a
/// roll's time left within half a tick.
fn slot_agrees(ours: ItemSlot, theirs: ItemSlot) -> bool {
    match (ours, theirs) {
        (
            ItemSlot::Rolling {
                item: ours,
                roll_left_s: ours_s,
            },
            ItemSlot::Rolling {
                item: theirs,
                roll_left_s: theirs_s,
            },
        ) => ours == theirs && (ours_s - theirs_s).abs() < TICK_S / 2.0,
        _ => ours == theirs,
    }
}

/// Whether the snapshot puts the kart where the record does, to the tuned
/// tolerance: its position, its nose, the tick of travel its velocity and
/// its bump make, its drift, its item, its coins, its tyre life, its spin-out, its stall,
/// its star's run, its place in the lap and, once `racing`, its rocket
/// start's verdict; during the countdown the start is not compared, since
/// a kart adopts it only from Go.
fn agrees(record: &KartRecord, kart: &KartReadout, racing: bool) -> bool {
    let nose = |rotation: Rotation| rotation * Vec3::NEG_Z * KART_HALF_LENGTH_M;
    let near = |a: Vec3, b: Vec3| a.distance(b) <= RECONCILIATION_TOLERANCE_M;
    let drift = match (record.kart.handling.drift(), kart.drift) {
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
        && near(
            record.kart.handling.bump() * TICK_S,
            kart.bump.map_or(Vec3::ZERO, Vec3::from) * TICK_S,
        )
        && drift
        && slot_agrees(record.kart.item_slot, kart.item_slot)
        && record.kart.coins == kart.coins
        && (record.kart.handling.tyre_life() - kart.tyre_life).abs()
            <= RECONCILIATION_TYRE_LIFE_TOLERANCE_PERCENT
        && (record.kart.spin_out_left_s.max(0.0) - kart.spin_out_s.unwrap_or(0.0)).abs()
            < TICK_S / 2.0
        && (record.kart.stall_left_s.max(0.0) - kart.stall_s.unwrap_or(0.0)).abs() < TICK_S / 2.0
        && (record.kart.star_left_s.max(0.0) - kart.star_s.unwrap_or(0.0)).abs() < TICK_S / 2.0
        && slipstream_agrees(record.kart.handling.slipstream(), kart.slipstream)
        && (!racing || record.kart.start == kart.start)
        && record.kart.lap() == kart.lap
        && record.kart.checkpoint == kart.checkpoint
}

/// Whether the snapshot's slipstream is the record's: the same state, with
/// a charge or a tow's time within half a tick. The prediction carries a
/// snapshot's slipstream on as the server does while the kart stays where
/// the snapshot had it, so this disagrees only as the kart enters or
/// leaves a wake, and a rewind then adopts the server's word.
fn slipstream_agrees(ours: Option<Slipstream>, theirs: Option<Slipstream>) -> bool {
    match (ours, theirs) {
        (None, None) => true,
        (Some(Slipstream::Charging { s: ours }), Some(Slipstream::Charging { s: theirs }))
        | (Some(Slipstream::Tow { left_s: ours }), Some(Slipstream::Tow { left_s: theirs })) => {
            (ours - theirs).abs() < TICK_S / 2.0
        }
        _ => false,
    }
}

impl KartRecord {
    /// The snapshot's word on everything it carries, at `clock`, the race
    /// clock at the snapshot's tick: the body where it has it and the kart
    /// as [`Kart::adopt`] takes it on.
    fn take(&mut self, snapshot: &KartReadout, clock: Phase) {
        self.translation = Vec3::from(snapshot.position);
        self.rotation = Rotation::from_array(snapshot.rotation).normalize();
        self.linvel = Vec3::from(snapshot.velocity);
        self.kart.adopt(snapshot, clock);
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
