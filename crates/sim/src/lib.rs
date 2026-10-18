//! The rules of the game, run by the browser (through `bridge`) and the
//! server. Must stay free of I/O, async runtimes and anything else from this
//! repo so both sides run identical rules.
//!
//! The public seam is [`step`]: feed a [`State`] one tick of [`KartInput`]
//! per kart and get the next state back. Everything a renderer needs is read
//! through [`State::kart_transforms`]. A race runs on a [`Track`] loaded from
//! the sidecar exported next to its glTF. A [`Replay`] reruns a race and
//! [`State::hash`] tells whether two runs agree.
//!
//! Bots live here too (ADR 0002): a race built with [`State::with_bots`]
//! puts bot karts on the grid behind the humans, and `step` has [`bot`]
//! drive them from the same readouts and into the same input struct a human
//! uses, before it applies anyone's input.

pub mod bot;
mod hash;
pub mod replay;
pub mod track;
pub mod tuning;

pub use replay::{Replay, ReplayError, Segment};
pub use track::{Checkpoint, Placement, Surface, SurfaceMesh, Track, TrackError};

use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::Hasher;

/// One kart's controls for one tick, already mapped from whatever device
/// produced them. Bots and humans both speak this. The buttons default to
/// unpressed so a replay recorded before they existed still runs.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KartInput {
    /// -1 is full left, 1 is full right.
    pub steer: f32,
    /// 0 to 1.
    pub throttle: f32,
    /// 0 to 1. Held on its own past a standstill, it backs the kart up.
    pub brake: f32,
    /// The hop button is down. A fresh press on the ground hops; held
    /// through the landing it keeps the drift, and letting go releases the
    /// mini-turbo.
    #[serde(default)]
    pub hop: bool,
    /// The item button went down this tick. Carried through the bridge and
    /// recordings already; `step` does not read the item buttons until
    /// items become rules.
    #[serde(default)]
    pub use_item: bool,
    /// The item button is down, this tick included.
    #[serde(default)]
    pub hold_item: bool,
}

/// What the View and the Instrument panel need per kart: where it is, in
/// metres and as a unit quaternion `[x, y, z, w]`, how fast it is going, and
/// the drift under way. Y is up and a kart with identity rotation faces -Z,
/// matching Three.js.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KartTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    /// Metres per second along the kart's facing; negative when backing up.
    pub speed: f32,
    /// The drift the kart is in; `None` when it is not drifting.
    pub drift: Option<Drift>,
}

/// Which way a drift turns the kart. Fixed by the steer at the hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftDirection {
    Left,
    Right,
}

impl DriftDirection {
    /// The steer value that turns this way, which is also how the bridge
    /// carries the direction.
    pub fn steer_sign(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }

    fn from_steer(steer: f32) -> Option<Self> {
        if steer < 0.0 {
            Some(Self::Left)
        } else if steer > 0.0 {
            Some(Self::Right)
        } else {
            None
        }
    }
}

/// A drift under way: its direction and how long it has been held, which
/// is what the mini-turbo tier is read from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drift {
    pub direction: DriftDirection,
    /// Seconds the drift has been held.
    pub charge_s: f32,
}

impl Drift {
    /// The mini-turbo tier the charge has reached: the boost a release now
    /// would give. Sparks at the wheels show it.
    pub fn mini_turbo(&self) -> MiniTurbo {
        use tuning::{MINI_TURBO_BLUE_CHARGE_S, MINI_TURBO_ORANGE_CHARGE_S};
        if self.charge_s >= MINI_TURBO_ORANGE_CHARGE_S {
            MiniTurbo::Orange
        } else if self.charge_s >= MINI_TURBO_BLUE_CHARGE_S {
            MiniTurbo::Blue
        } else {
            MiniTurbo::None
        }
    }
}

/// The mini-turbo tiers a drift charges through, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniTurbo {
    None,
    Blue,
    Orange,
}

impl MiniTurbo {
    /// The tier as one byte in charge order, for the hash and the bridge.
    pub fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Blue => 1,
            Self::Orange => 2,
        }
    }

    /// How long releasing this tier boosts the kart.
    fn boost_s(self) -> f32 {
        use tuning::{MINI_TURBO_BLUE_BOOST_S, MINI_TURBO_ORANGE_BOOST_S};
        match self {
            Self::None => 0.0,
            Self::Blue => MINI_TURBO_BLUE_BOOST_S,
            Self::Orange => MINI_TURBO_ORANGE_BOOST_S,
        }
    }
}

/// Where a kart stands in the race: what the Instrument panel shows and
/// what race position is ordered by. One per kart from [`State::standings`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    /// Race position, 1 for the leader. Ordered by laps completed, then
    /// checkpoint, then spline progress; a dead heat keeps grid order.
    pub position: u32,
    pub laps_completed: u32,
    /// The last checkpoint crossed in order this lap; 0 is the start line,
    /// which the grid counts as having crossed.
    pub checkpoint: usize,
    /// Metres along the centreline from the start line, in `0..lap length`.
    pub spline_progress: f32,
    /// Seconds since this lap began; the first lap begins at the first tick.
    pub lap_time_s: f32,
    /// The fastest lap completed so far.
    pub best_lap_s: Option<f32>,
    /// Spline progress has been falling for the tuned time: the kart is
    /// going the wrong way. Stays up until progress rises again.
    pub wrong_way: bool,
}

impl Standing {
    /// The lap the kart is on, from 1; a kart that has completed the race's
    /// laps stays on the last one.
    pub fn lap(&self) -> u32 {
        (self.laps_completed + 1).min(LAPS_PER_RACE)
    }
}

/// Everything the sim knows about a race. Owns the physics world and the
/// RNG, so a `State` plus an input sequence replays identically anywhere.
pub struct State {
    rng: ChaCha8Rng,
    track: Track,
    /// Human karts first, then bots: `step`'s inputs are the humans', in
    /// grid order.
    karts: Vec<Kart>,
    /// The first this many `karts` are humans; fixed for the race.
    humans: usize,
    physics: Physics,
}

struct Kart {
    body: RigidBodyHandle,
    collider: ColliderHandle,
    /// The bot driving this kart, with what it remembers; `None` for a
    /// human's kart.
    bot: Option<bot::Memory>,
    /// The unit direction the handling model sent the kart this tick: its
    /// facing, turned by the slip angle in a drift. What a wall bump
    /// measures the surviving speed along.
    travel: Vec3,
    /// Unit normals, pointing at the kart, of the walls it ended the last
    /// tick against. The next tick's handling sends no velocity into them.
    walls: Vec<Vec3>,
    /// Seconds the kart has been outside the world without a break; it
    /// respawns when this reaches the tuned delay.
    out_of_world_s: f32,
    /// Seconds spline progress has been falling without rising; the
    /// wrong-way warning shows once this reaches the tuned time. Standing
    /// still neither adds to it nor clears it.
    wrong_way_s: f32,
    /// Speed along the kart's facing, metres per second. Held outside Rapier
    /// so contacts and friction never leak into the handling model.
    speed: f32,
    /// Seconds of hop airtime left; zero or less on the ground. A timer
    /// rather than a Rapier contact, so the landing tick never depends on
    /// contact tolerances.
    airtime_left_s: f32,
    /// Whether the hop button was down last tick, so a hop needs a fresh
    /// press: holding the button through a landing never hops again.
    hop_was_held: bool,
    /// The direction the kart was steering when it hopped: the direction a
    /// drift on landing takes. `None` when it hopped straight.
    hop_direction: Option<DriftDirection>,
    drift: Option<Drift>,
    /// Whether the kart was on a boost pad last tick, so a pad fires once,
    /// as the kart enters it, and its clock runs from there.
    on_boost_pad: bool,
    /// Seconds of boost left; zero or less when not boosting.
    boost_left_s: f32,
    /// Top speed multiplier while boosting: the mini-turbo's or the boost
    /// pad's, whichever started last.
    boost_factor: f32,
    laps_completed: u32,
    /// The last checkpoint crossed in order this lap. The grid starts at 0,
    /// the start line, so the first lap begins at the first tick.
    checkpoint: usize,
    spline_progress: f32,
    lap_time_s: f32,
    best_lap_s: Option<f32>,
}

impl Kart {
    fn airborne(&self) -> bool {
        self.airtime_left_s > 0.0
    }
}

/// Fast enough to slide: what a landing needs to start a drift and a drift
/// needs to go on.
fn fast_enough_to_drift(speed: f32) -> bool {
    speed > DRIFT_ENTRY_SPEED_FRACTION * TOP_SPEED_M_PER_S
}

/// Rapier's working sets. One struct so `step` passes them around as a unit.
struct Physics {
    pipeline: PhysicsPipeline,
    integration: IntegrationParameters,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    soft_bodies: SoftBodySet,
    ccd: CCDSolver,
}

use tuning::{BOOST_PAD_FACTOR, BOOST_PAD_S, RESPAWN_DELAY_S, WRONG_WAY_WARNING_S};
use tuning::{BOT_RUBBER_BAND_MAX_FACTOR, BOT_RUBBER_BAND_MIN_FACTOR, BOT_RUBBER_BAND_RANGE_M};
use tuning::{
    DRIFT_ENTRY_SPEED_FRACTION, GRAVITY_M_PER_S2, HOP_AIRTIME_S, KART_EDGE_RADIUS_M,
    KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M, LAPS_PER_RACE,
    MINI_TURBO_BOOST_FACTOR, OFF_TRACK_SPEED_FACTOR, TOP_SPEED_M_PER_S,
};

/// A surface query starts this far above the asked point, so a point lying
/// exactly on a surface still finds it despite rounding.
const SURFACE_PROBE_RISE_M: f32 = 0.1;

/// How far ahead of a kart Rapier looks for contacts: past the farthest a
/// kart moves in one tick at the biggest boost, so no wall is entered
/// before it is seen.
const WALL_PREDICTION_M: f32 =
    2.0 * BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S / tuning::TICKS_PER_SECOND as f32;

/// Stands in for a missing best lap in the hash; a lap time is never negative.
const NO_BEST_LAP: f32 = -1.0;

impl State {
    /// A fresh race on `track` with `kart_count` human karts on its spawn
    /// grid, in slot order. `kart_count` must not exceed the track's spawn
    /// slots.
    pub fn new(seed: u64, kart_count: usize, track: Track) -> Self {
        Self::with_bots(seed, kart_count, 0, track)
    }

    /// A fresh race on `track` with `humans` karts on the front of its spawn
    /// grid and `bots` karts on the slots behind them. Together they must
    /// not exceed the track's spawn slots.
    pub fn with_bots(seed: u64, humans: usize, bots: usize, track: Track) -> Self {
        let kart_count = humans + bots;
        assert!(
            kart_count <= track.spawn_grid().len(),
            "{} karts but the track has {} spawn slots",
            kart_count,
            track.spawn_grid().len()
        );
        let mut physics = Physics::new();
        let ground = physics.bodies.insert(RigidBodyBuilder::fixed().build());
        for (surface, shape) in track.colliders() {
            physics.colliders.insert_with_parent(
                ColliderBuilder::new(shape.clone())
                    .friction(0.0)
                    .user_data(u128::from(surface.code()))
                    .build(),
                ground,
                &mut physics.bodies,
            );
        }

        let karts = track.spawn_grid()[..kart_count]
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let body = RigidBodyBuilder::dynamic()
                    .translation(Vec3::from(slot.position) + Vec3::Y * KART_HALF_HEIGHT_M)
                    .rotation(Vec3::Y * slot.yaw)
                    // Steering is a yaw we set each tick; the box must never
                    // tip or roll from a contact.
                    .enabled_rotations(false, true, false)
                    // Contacts are predicted this far ahead, so a kart
                    // meets a wall where it stands instead of inside it.
                    .soft_ccd_prediction(WALL_PREDICTION_M)
                    .build();
                let body = physics.bodies.insert(body);
                // The body's length runs along -Z, its facing.
                let collider = physics.colliders.insert_with_parent(
                    ColliderBuilder::round_cuboid(
                        KART_HALF_WIDTH_M - KART_EDGE_RADIUS_M,
                        KART_HALF_HEIGHT_M - KART_EDGE_RADIUS_M,
                        KART_HALF_LENGTH_M - KART_EDGE_RADIUS_M,
                        KART_EDGE_RADIUS_M,
                    )
                    .friction(0.0)
                    .restitution(0.0)
                    .build(),
                    body,
                    &mut physics.bodies,
                );
                Kart {
                    body,
                    collider,
                    bot: (index >= humans).then_some(bot::Memory::default()),
                    travel: Vec3::NEG_Z,
                    walls: Vec::new(),
                    out_of_world_s: 0.0,
                    wrong_way_s: 0.0,
                    speed: 0.0,
                    airtime_left_s: 0.0,
                    hop_was_held: false,
                    hop_direction: None,
                    drift: None,
                    on_boost_pad: false,
                    boost_left_s: 0.0,
                    boost_factor: 1.0,
                    laps_completed: 0,
                    checkpoint: 0,
                    spline_progress: track.progress_at(slot.position),
                    lap_time_s: 0.0,
                    best_lap_s: None,
                }
            })
            .collect();
        physics.index_colliders();

        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            track,
            karts,
            humans,
            physics,
        }
    }

    pub fn track(&self) -> &Track {
        &self.track
    }

    /// Karts driven from `step`'s inputs: the first this many grid slots.
    pub fn human_count(&self) -> usize {
        self.humans
    }

    /// Whether the kart in `slot` is driven by a bot.
    pub fn is_bot(&self, slot: usize) -> bool {
        self.karts[slot].bot.is_some()
    }

    /// The rubber band on a kart's top speed for the coming tick: a bot the
    /// tuned range or more ahead of the nearest human along the spline is
    /// held to the tuned minimum, one that far behind is let up to the tuned
    /// maximum, and the scale runs linearly between. A human's kart, or a
    /// bot in a race with no humans, is scaled by one.
    pub fn rubber_band(&self, slot: usize) -> f32 {
        if !self.is_bot(slot) || self.humans == 0 {
            return 1.0;
        }
        let distance = |kart: &Kart| {
            kart.laps_completed as f32 * self.track.spline_length()
                + self
                    .track
                    .lap_relative_progress(kart.checkpoint, kart.spline_progress)
        };
        let bot = distance(&self.karts[slot]);
        let ahead_of_nearest = self.karts[..self.humans]
            .iter()
            .map(|human| bot - distance(human))
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .expect("a race with humans has a nearest one");
        let fraction = (ahead_of_nearest / BOT_RUBBER_BAND_RANGE_M).clamp(-1.0, 1.0);
        if fraction > 0.0 {
            1.0 - fraction * (1.0 - BOT_RUBBER_BAND_MIN_FACTOR)
        } else {
            1.0 - fraction * (BOT_RUBBER_BAND_MAX_FACTOR - 1.0)
        }
    }

    /// What lies under `point`: the topmost tagged surface at or below it,
    /// or `OutOfWorld` when there is none. Karts are never an answer.
    pub fn surface_at(&self, point: [f32; 3]) -> Surface {
        self.physics.surface_at(point)
    }

    /// The surface under a kart's centre.
    pub fn kart_surface(&self, kart: usize) -> Surface {
        self.surface_at(self.kart_transforms()[kart].position)
    }

    /// A hash of everything that influences future ticks: the RNG and, per
    /// kart, its speed and its body's position, heading, velocities and
    /// sleep state, including the timer that puts a resting body to sleep.
    /// Bit-identical on every platform for bit-identical states, which is
    /// what the replay fixtures check (ADR 0002). A bot's memory is hashed
    /// with its kart. Three things are left out on purpose. The track and
    /// the split between human and bot slots never change after `new`, so
    /// they are the replay's `track` and `bots` fields instead. Rapier's
    /// contact manifolds and warm-start impulses are not hashed; a
    /// difference there reaches a body's velocity on the next tick and is
    /// caught then.
    pub fn hash(&self) -> u64 {
        let mut hasher = hash::Fnv1a::new();
        hasher.write(&self.rng.get_seed());
        hasher.write_u64(self.rng.get_stream());
        hasher.write_u128(self.rng.get_word_pos());
        for kart in &self.karts {
            let body = &self.physics.bodies[kart.body];
            hasher.write_f32(kart.speed);
            hasher.write_f32(kart.airtime_left_s);
            hasher.write_u8(u8::from(kart.hop_was_held));
            hasher.write_f32(kart.hop_direction.map_or(0.0, DriftDirection::steer_sign));
            hasher.write_f32(kart.drift.map_or(0.0, |drift| drift.direction.steer_sign()));
            hasher.write_f32(kart.drift.map_or(0.0, |drift| drift.charge_s));
            hasher.write_u8(u8::from(kart.on_boost_pad));
            hasher.write_f32(kart.boost_left_s);
            hasher.write_f32(kart.boost_factor);
            hasher.write_u64(kart.walls.len() as u64);
            for wall in &kart.walls {
                hasher.write_f32s(wall.to_array());
            }
            hasher.write_f32(kart.out_of_world_s);
            hasher.write_f32(kart.wrong_way_s);
            hasher.write_u32(kart.laps_completed);
            hasher.write_u64(kart.checkpoint as u64);
            hasher.write_f32(kart.spline_progress);
            hasher.write_f32(kart.lap_time_s);
            hasher.write_f32(kart.best_lap_s.unwrap_or(NO_BEST_LAP));
            if let Some(memory) = &kart.bot {
                hasher.write_f32s(memory.timers());
            }
            hasher.write_f32s(body.translation().to_array());
            hasher.write_f32s(body.rotation().to_array());
            hasher.write_f32s(body.linvel().to_array());
            hasher.write_f32s(body.angvel().to_array());
            hasher.write_f32(body.activation().time_since_can_sleep);
            hasher.write_u8(u8::from(body.is_sleeping()));
        }
        hasher.finish()
    }

    /// One standing per kart, in grid order, with race positions as of the
    /// last tick so an overtake shows the tick it happens.
    pub fn standings(&self) -> Vec<Standing> {
        let mut order: Vec<usize> = (0..self.karts.len()).collect();
        // Stable, so karts level on every key keep grid order.
        order.sort_by(|&a, &b| {
            let (a, b) = (&self.karts[a], &self.karts[b]);
            b.laps_completed
                .cmp(&a.laps_completed)
                .then(b.checkpoint.cmp(&a.checkpoint))
                .then(
                    self.track
                        .lap_relative_progress(b.checkpoint, b.spline_progress)
                        .total_cmp(
                            &self
                                .track
                                .lap_relative_progress(a.checkpoint, a.spline_progress),
                        ),
                )
        });
        let mut positions = vec![0; self.karts.len()];
        for (place, &slot) in order.iter().enumerate() {
            positions[slot] = place as u32 + 1;
        }
        self.karts
            .iter()
            .zip(positions)
            .map(|(kart, position)| Standing {
                position,
                laps_completed: kart.laps_completed,
                checkpoint: kart.checkpoint,
                spline_progress: kart.spline_progress,
                lap_time_s: kart.lap_time_s,
                best_lap_s: kart.best_lap_s,
                wrong_way: kart.wrong_way_s >= WRONG_WAY_WARNING_S,
            })
            .collect()
    }

    /// One transform per kart, in grid order.
    pub fn kart_transforms(&self) -> Vec<KartTransform> {
        self.karts
            .iter()
            .map(|kart| {
                let body = &self.physics.bodies[kart.body];
                let position = body.translation();
                let rotation = body.rotation();
                KartTransform {
                    position: [position.x, position.y, position.z],
                    rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
                    speed: kart.speed,
                    drift: kart.drift,
                }
            })
            .collect()
    }
}

/// Advance the race by one tick of `dt` seconds. `inputs` holds one entry per
/// human kart in grid order; the caller must supply exactly as many as there
/// are humans. The bots' inputs come from [`bot::drive`], given the readouts
/// as of the last tick.
pub fn step(mut state: State, inputs: &[KartInput], dt: f32) -> State {
    assert_eq!(inputs.len(), state.humans, "one input per human kart");
    // Skip building the readouts when no bot needs them.
    let inputs: Vec<KartInput> = if state.humans == state.karts.len() {
        inputs.to_vec()
    } else {
        let transforms = state.kart_transforms();
        let standings = state.standings();
        let track = &state.track;
        let mut inputs = inputs.to_vec();
        for (slot, kart) in state.karts.iter_mut().enumerate() {
            if let Some(memory) = &mut kart.bot {
                inputs.push(bot::drive(track, &transforms, &standings, slot, memory, dt));
            }
        }
        inputs
    };
    let top_speeds: Vec<f32> = (0..state.karts.len())
        .map(|slot| TOP_SPEED_M_PER_S * state.rubber_band(slot))
        .collect();
    let surfaces: Vec<Surface> = state
        .karts
        .iter()
        .map(|kart| {
            state
                .physics
                .surface_at(state.physics.bodies[kart.body].translation().to_array())
        })
        .collect();
    for (((kart, input), surface), top_speed) in state
        .karts
        .iter_mut()
        .zip(&inputs)
        .zip(surfaces)
        .zip(top_speeds)
    {
        let body = &mut state.physics.bodies[kart.body];
        let position = body.translation().to_array();
        // Keep whatever vertical velocity gravity and the plane have agreed
        // on, unless this tick's hop launches the kart.
        let mut vertical = body.linvel().y;
        if kart.airborne() {
            kart.airtime_left_s -= dt;
            if !kart.airborne() {
                kart.drift = landing_drift(kart, input);
            }
        } else if let Some(drift) = &mut kart.drift {
            if input.hop && fast_enough_to_drift(kart.speed) {
                drift.charge_s += dt;
            } else {
                // Letting go releases what the drift charged; losing the
                // speed to slide just ends it.
                if !input.hop {
                    kart.boost_left_s = drift.mini_turbo().boost_s();
                    kart.boost_factor = MINI_TURBO_BOOST_FACTOR;
                }
                kart.drift = None;
            }
        } else if input.hop && !kart.hop_was_held {
            kart.airtime_left_s = HOP_AIRTIME_S;
            kart.hop_direction = DriftDirection::from_steer(input.steer);
            vertical = hop_launch_speed();
        }
        kart.hop_was_held = input.hop;
        kart.speed = next_speed(kart.speed, input, dt, top_speed);
        // Off the asphalt the kart is held to the cap, not dragged down to
        // it, so it slows the tick it leaves the track. The cap reads the
        // surface below the kart, so a hop over a patch does not clear it.
        if surface.slows_karts() {
            kart.speed = kart.speed.min(OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S);
        }
        let on_boost_pad = !kart.airborne() && state.track.on_a_boost_pad(position);
        if on_boost_pad && !kart.on_boost_pad {
            kart.boost_left_s = BOOST_PAD_S;
            kart.boost_factor = BOOST_PAD_FACTOR;
        }
        kart.on_boost_pad = on_boost_pad;
        if kart.boost_left_s > 0.0 {
            kart.boost_left_s -= dt;
            kart.speed = TOP_SPEED_M_PER_S * kart.boost_factor;
        }
        let (steer, slip) = match kart.drift {
            Some(drift) => (
                drift_steer(drift.direction, input.steer),
                drift_slip(drift.direction),
            ),
            None => (input.steer.clamp(-1.0, 1.0), 0.0),
        };
        let forward = *body.rotation() * Rotation::from_rotation_y(slip) * Vec3::NEG_Z;
        // A wall the kart is against takes the velocity into it, and the
        // speed with it, before the solver sees either; otherwise the kart
        // would press into the wall a little every tick.
        let mut velocity = forward * kart.speed;
        if !kart.walls.is_empty() {
            velocity = stopped_by_walls(velocity, &kart.walls);
            kart.speed = forward.dot(velocity);
        }
        body.set_linvel(velocity + Vec3::Y * vertical, true);
        body.set_angvel(Vec3::Y * yaw_rate(kart.speed, steer), true);
        kart.travel = forward;
    }
    let before: Vec<[f32; 3]> = state
        .karts
        .iter()
        .map(|kart| state.physics.bodies[kart.body].translation().to_array())
        .collect();
    state.physics.step(dt);
    for (kart, mut before) in state.karts.iter_mut().zip(before) {
        bump_off_walls(kart, &mut state.physics, dt);
        let mut after = state.physics.bodies[kart.body].translation().to_array();
        if state.physics.surface_at(after) == Surface::OutOfWorld {
            kart.out_of_world_s += dt;
            if kart.out_of_world_s >= RESPAWN_DELAY_S {
                after = respawn(kart, &state.track, &mut state.physics);
                // A teleport is not a move: it crosses no gate.
                before = after;
            }
        } else {
            kart.out_of_world_s = 0.0;
        }
        let progress = state.track.progress_at(after);
        // Progress wraps at the start line; the shorter way round says
        // which way the kart went.
        let moved = state.track.lap_relative_progress(kart.checkpoint, progress)
            - state
                .track
                .lap_relative_progress(kart.checkpoint, kart.spline_progress);
        if moved < 0.0 {
            kart.wrong_way_s += dt;
        } else if moved > 0.0 {
            kart.wrong_way_s = 0.0;
        }
        kart.spline_progress = progress;
        kart.lap_time_s += dt;
        cross_checkpoint(kart, &state.track, before, after, dt);
    }
    state
}

/// A contact point this close to a wall at the end of the tick counts as
/// touching it; contacts further off are the solver's look-ahead.
const WALL_TOUCH_M: f32 = 0.01;

/// `velocity` with its component into each of `walls` removed, so the
/// motion into a wall stops and the motion along it is kept. Motion away
/// from a wall is left alone.
fn stopped_by_walls(mut velocity: Vec3, walls: &[Vec3]) -> Vec3 {
    for &normal in walls {
        let into = velocity.dot(normal);
        if into < 0.0 {
            velocity -= normal * into;
        }
    }
    velocity
}

/// The wall stops the kart with a bump and no bounce. Every wall the kart
/// ends the tick against is remembered for the next tick's handling, and a
/// wall met this tick takes the velocity into it there and then: the
/// velocity along the wall is kept and the kart's speed becomes what is
/// left along its travel. A kart facing straight at a wall stops dead; one
/// scraping along at an angle loses the share of its speed that pointed
/// into the wall each tick, so it slows until it steers away.
fn bump_off_walls(kart: &mut Kart, physics: &mut Physics, dt: f32) {
    let velocity = physics.bodies[kart.body].linvel();
    kart.walls.clear();
    for pair in physics.narrow_phase.contact_pairs_with(kart.collider) {
        let other = if pair.collider1 == kart.collider {
            pair.collider2
        } else {
            pair.collider1
        };
        if physics.colliders[other].user_data != u128::from(Surface::Wall.code()) {
            continue;
        }
        for manifold in pair.manifolds() {
            // Rapier's normal points from the first collider to the second;
            // the rule wants it pointing out of the wall, toward the kart.
            let normal = if pair.collider1 == kart.collider {
                -manifold.data.normal
            } else {
                manifold.data.normal
            };
            // Contact distances are from the start of the tick; the kart's
            // velocity says where it ended up.
            let closing = velocity.dot(normal) * dt;
            if manifold
                .points
                .iter()
                .any(|contact| contact.dist + closing <= WALL_TOUCH_M)
            {
                kart.walls.push(normal);
            }
        }
    }
    let stopped = stopped_by_walls(velocity, &kart.walls);
    if stopped != velocity {
        physics.bodies[kart.body].set_linvel(stopped, true);
        kart.speed = kart.travel.dot(stopped);
    }
}

/// Put the kart back on the track at its last checkpoint, at rest and
/// facing the next one, with nothing carried over from before it fell.
/// Returns where it now stands.
fn respawn(kart: &mut Kart, track: &Track, physics: &mut Physics) -> [f32; 3] {
    let checkpoints = track.checkpoints();
    let here = checkpoints[kart.checkpoint].position;
    let next = checkpoints[(kart.checkpoint + 1) % checkpoints.len()].position;
    // Yaw zero faces -Z and grows counter-clockwise seen from above.
    let yaw = libm::atan2f(here[0] - next[0], here[2] - next[2]);
    let facing = Rotation::from_rotation_y(yaw);
    let position = Vec3::from(here) + Vec3::Y * KART_HALF_HEIGHT_M;
    let body = &mut physics.bodies[kart.body];
    body.set_translation(position, true);
    body.set_rotation(facing, true);
    body.set_linvel(Vec3::ZERO, true);
    body.set_angvel(Vec3::ZERO, true);
    kart.speed = 0.0;
    kart.airtime_left_s = 0.0;
    kart.hop_direction = None;
    kart.drift = None;
    kart.on_boost_pad = false;
    kart.boost_left_s = 0.0;
    kart.travel = facing * Vec3::NEG_Z;
    kart.walls.clear();
    kart.out_of_world_s = 0.0;
    kart.wrong_way_s = 0.0;
    position.to_array()
}

/// Credit the kart with the checkpoint its move from `before` to `after`
/// crossed, when it is the next in order; a lap completes at the start
/// line after the last one. The lap's time ends where the move met the
/// line, so lap times are finer than a tick, and the next lap's clock
/// starts with the rest of the tick.
fn cross_checkpoint(kart: &mut Kart, track: &Track, before: [f32; 3], after: [f32; 3], dt: f32) {
    let checkpoints = track.checkpoints();
    let next = (kart.checkpoint + 1) % checkpoints.len();
    let Some(fraction) = checkpoints[next].crossed_between(before, after) else {
        return;
    };
    if next == 0 {
        let lap_time_s = kart.lap_time_s - dt * (1.0 - fraction);
        kart.laps_completed += 1;
        kart.best_lap_s = Some(
            kart.best_lap_s
                .map_or(lap_time_s, |best| best.min(lap_time_s)),
        );
        kart.lap_time_s = dt * (1.0 - fraction);
    }
    kart.checkpoint = next;
}

/// The upward speed that brings a kart back to the track after the tuned
/// airtime under the tuned gravity, so the body's arc agrees with the timer.
fn hop_launch_speed() -> f32 {
    -GRAVITY_M_PER_S2 * HOP_AIRTIME_S / 2.0
}

/// The drift a landing starts: the hop's direction, when the kart hopped
/// while steering, is still steering and holding the hop button as it
/// lands, and is going fast enough to slide.
fn landing_drift(kart: &Kart, input: &KartInput) -> Option<Drift> {
    let direction = kart.hop_direction?;
    (input.hop && input.steer != 0.0 && fast_enough_to_drift(kart.speed)).then_some(Drift {
        direction,
        charge_s: 0.0,
    })
}

/// Speed after one tick: throttle ramps toward `top_speed` and brake toward
/// a standstill, each at its tuned rate, and the brake on its own carries on
/// into reverse. With neither held the kart coasts toward a standstill and
/// stays there. `top_speed` is the cap under the kart's rubber band; the
/// ramp rates stay the tuned top speed's.
fn next_speed(speed: f32, input: &KartInput, dt: f32, top_speed: f32) -> f32 {
    use tuning::{
        ACCELERATION_TO_TOP_S, BRAKE_TO_STOP_S, COAST_TO_STOP_S, REVERSE_TOP_SPEED_M_PER_S,
        TOP_SPEED_M_PER_S,
    };
    let throttle = input.throttle.clamp(0.0, 1.0);
    let brake = input.brake.clamp(0.0, 1.0);
    let accelerate = throttle * TOP_SPEED_M_PER_S / ACCELERATION_TO_TOP_S;
    let braking = brake * TOP_SPEED_M_PER_S / BRAKE_TO_STOP_S;
    let coasted = if throttle == 0.0 && brake == 0.0 {
        let coast = TOP_SPEED_M_PER_S / COAST_TO_STOP_S * dt;
        // Toward zero from either side, never through it.
        speed - speed.clamp(-coast, coast)
    } else {
        speed
    };
    // With the throttle held the brake stops the kart and holds it; it never
    // backs up a kart that was going forward, and never speeds up one that
    // was already backing up.
    let floor = if throttle > 0.0 {
        speed.min(0.0)
    } else {
        -REVERSE_TOP_SPEED_M_PER_S
    };
    (coasted + (accelerate - braking) * dt).clamp(floor, top_speed)
}

/// The steer a drift turns with, in the units of `KartInput::steer` but
/// past full lock when the player steers into it: the drift's own direction
/// at the tuned neutral strength, tightened by inward steer and widened by
/// counter-steer, never reversed.
fn drift_steer(direction: DriftDirection, steer: f32) -> f32 {
    use tuning::{DRIFT_TURN_NEUTRAL, DRIFT_TURN_STEER_RANGE};
    let inward = (steer * direction.steer_sign()).clamp(-1.0, 1.0);
    direction.steer_sign() * (DRIFT_TURN_NEUTRAL + DRIFT_TURN_STEER_RANGE * inward)
}

/// The angle between where a drifting kart points and where it travels:
/// the nose points into the corner, the kart slides toward the outside.
/// Steering right is a negative rotation about +Y, so the outside of a
/// right drift is a positive one.
fn drift_slip(direction: DriftDirection) -> f32 {
    direction.steer_sign() * tuning::DRIFT_SLIP_ANGLE_RAD
}

/// Yaw rate for a steer value: positive steer turns a kart moving forward
/// right, which is a negative rotation about +Y, and swings the nose of a
/// kart backing up the other way, as a car's does. Faster karts steer slower,
/// and a near-stationary kart barely turns at all. Full lock is 1; a drift
/// may ask for more.
fn yaw_rate(speed: f32, steer: f32) -> f32 {
    use tuning::STEER_FULL_EFFECT_SPEED_M_PER_S;
    let grip = (speed.abs() / STEER_FULL_EFFECT_SPEED_M_PER_S).clamp(0.0, 1.0);
    -steer * steer_rate(speed) * grip * speed.signum()
}

/// The yaw rate full lock gives at `speed`: the tuned low-speed rate
/// falling to the tuned top-speed rate. Shared with the bot, so its model
/// of the handling is the handling.
pub(crate) fn steer_rate(speed: f32) -> f32 {
    use tuning::{
        STEER_RATE_LOW_SPEED_RAD_PER_S, STEER_RATE_TOP_SPEED_RAD_PER_S, TOP_SPEED_M_PER_S,
    };
    let speed_fraction = (speed.abs() / TOP_SPEED_M_PER_S).clamp(0.0, 1.0);
    STEER_RATE_LOW_SPEED_RAD_PER_S
        + (STEER_RATE_TOP_SPEED_RAD_PER_S - STEER_RATE_LOW_SPEED_RAD_PER_S) * speed_fraction
}

impl Physics {
    fn new() -> Self {
        Self {
            pipeline: PhysicsPipeline::new(),
            integration: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd: CCDSolver::new(),
        }
    }

    /// The topmost tagged surface at or below `point`, or `OutOfWorld` when
    /// there is none. Karts are never an answer.
    fn surface_at(&self, point: [f32; 3]) -> Surface {
        let ray = Ray::new(
            Vec3::from(point) + Vec3::Y * SURFACE_PROBE_RISE_M,
            Vec3::NEG_Y,
        );
        let pipeline = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::only_fixed(),
        );
        pipeline
            .cast_ray(&ray, f32::MAX, true)
            .and_then(|(collider, _)| Surface::from_code(self.colliders[collider].user_data as u8))
            .unwrap_or(Surface::OutOfWorld)
    }

    /// Register newly inserted colliders with the broad phase so scene queries
    /// see the track before the first tick; `step` does this itself later.
    fn index_colliders(&mut self) {
        CollisionPipeline::new().step(
            self.integration.prediction_distance(),
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &(),
            &(),
        );
    }

    fn step(&mut self, dt: f32) {
        self.integration.dt = dt;
        self.pipeline.step(
            Vec3::new(0.0, GRAVITY_M_PER_S2, 0.0),
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            &mut self.ccd,
            &(),
            &(),
        );
    }
}
