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
//!
//! A race has a beginning and an end, and both are rules: [`State::phase`]
//! walks Countdown, Racing and Finished through `step`, so a room on the
//! server can drive the lifecycle a solo race in the browser does. What a
//! tick changed that the players are told about, a lap completed, a finish,
//! a respawn, an item picked up or fired, a spin-out, is read from
//! [`State::events`] after that tick.

pub mod bot;
mod hash;
pub mod interpolation;
pub mod prediction;
pub mod replay;
pub mod track;
pub mod tuning;

pub use interpolation::Interpolation;
pub use prediction::{Prediction, SnapshotKart};
pub use replay::{Replay, ReplayError, Segment};
pub use track::{Checkpoint, Placement, Surface, SurfaceMesh, Track, TrackError};

use rand_chacha::rand_core::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::Hasher;
use std::ops::Range;

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
    /// The item button went down this tick: fires the item the kart holds.
    #[serde(default)]
    pub use_item: bool,
    /// The item button is down, this tick included. Its first tick down
    /// fires too, so a press whose datagram was lost on the way to the
    /// server still fires from the next tick the button is seen held.
    #[serde(default)]
    pub hold_item: bool,
}

/// What the View and the Instrument panel need per kart: where it is, in
/// metres and as a unit quaternion `[x, y, z, w]`, how fast it is going,
/// the drift under way and the item it holds. Y is up and a kart with
/// identity rotation faces -Z, matching Three.js.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KartTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    /// Metres per second along the kart's facing; negative when backing up.
    pub speed: f32,
    /// The body's velocity in metres per second, which a snapshot carries:
    /// the speed along the travel, which a drift slips from the facing, and
    /// the vertical motion of a hop or a fall.
    pub velocity: [f32; 3],
    /// The drift the kart is in; `None` when it is not drifting.
    pub drift: Option<Drift>,
    /// The item in the kart's slot; `None` while the slot is empty.
    pub held_item: Option<Item>,
    /// Seconds of spin-out left, while the kart is spun out: stopped, with
    /// its inputs ignored. The View spins it for the duration.
    pub spin_out_s: Option<f32>,
    /// Whether the kart is in a hop's airtime; the hop's tell. Only the sim
    /// that steps the kart reports it: a snapshot does not carry it, so an
    /// interpolated kart is never airborne.
    pub airborne: bool,
    /// Seconds of boost left, while a mini-turbo, boost pad, item or start
    /// boost holds the kart over top speed; the release's tell. Only the sim
    /// that steps the kart reports it: a snapshot does not carry it, so an
    /// interpolated kart never boosts.
    pub boost_s: Option<f32>,
    /// The kart's rocket start: while the countdown runs the tier its hold
    /// so far has reached, the sparks' tell, and from Go the verdict.
    pub start: Start,
    /// Seconds of stall left, while the kart is stalled at Go: dead to
    /// every control, as in a spin-out. The sputter's tell.
    pub stall_s: Option<f32>,
}

/// The items a kart can hold: what an item box rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// A burst of speed: the tuned factor on top speed for the tuned time.
    Boost,
    /// Dropped behind the kart; spins out whoever hits it.
    Hazard,
    /// Follows the track to the kart ahead and spins it out.
    Homing,
}

/// A homing projectile in flight, as the View draws it: where it is, whose
/// it is and which kart it chases. It flies the spline at the tuned speed
/// toward the kart that was next ahead of its owner by race position when
/// it was fired, and is used up on reaching that kart: on a hazard the
/// kart trails, which goes with it, else on the kart, which spins out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projectile {
    /// Its centre, at the centreline's height where it is, in metres.
    pub position: [f32; 3],
    /// The grid index of the kart that fired it.
    pub owner: usize,
    /// The grid index of the kart it chases.
    pub target: usize,
}

/// A hazard on the track, as the View draws it: where it lies, whose it is
/// and whether that kart is still trailing it behind itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hazard {
    /// Its centre, on the surface, in metres.
    pub position: [f32; 3],
    /// The grid index of the kart that fired it.
    pub owner: usize,
    /// Trailing the tuned distance behind its owner while the owner holds
    /// the item button; dropped where it is once the button is let go.
    pub held: bool,
}

/// A hazard as the rules keep it: what the View sees, plus how long it has
/// left and whether its owner has got clear of it since the drop.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PlacedHazard {
    hazard: Hazard,
    /// Seconds until a dropped hazard is gone; a held one does not age.
    lifetime_left_s: f32,
    /// Whether the owner has been out of the hazard's reach since firing
    /// it. Until then the owner cannot hit it: the drop puts it right
    /// behind the kart, and a hold keeps it there.
    clear_of_owner: bool,
}

impl Item {
    /// The item as one byte, for the hash and the bridge; 0 is left for an
    /// empty slot.
    pub fn code(self) -> u8 {
        match self {
            Self::Boost => 1,
            Self::Hazard => 2,
            Self::Homing => 3,
        }
    }

    /// The item a roll in `0..ITEM_ROLL_PERCENT` picks for a kart in race `position`,
    /// from the tuned weights of the position's band: the first `hazard`
    /// percent are the hazard, the next `boost` the boost and the rest the
    /// homing projectile. A position past the last band rolls as the last.
    fn rolled(position: u32, roll: u32) -> Self {
        let band = ITEM_ROLL_WEIGHTS
            .iter()
            .find(|band| position <= u32::from(band.positions.1))
            .or(ITEM_ROLL_WEIGHTS.last())
            .expect("the roll table has a band");
        if roll < u32::from(band.hazard) {
            Self::Hazard
        } else if roll < u32::from(band.hazard) + u32::from(band.boost) {
            Self::Boost
        } else {
            Self::Homing
        }
    }
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

/// Something a tick changed that the players are told about. The server
/// turns these into the protocol's events; a client only shows them.
/// `kart` is the grid index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RaceEvent {
    /// The kart crossed the start line after every checkpoint: `lap` is the
    /// lap just completed, from 1, and `lap_time_s` its time.
    LapCompleted {
        kart: usize,
        lap: u32,
        lap_time_s: f32,
    },
    /// The kart completed the race's laps, taking `position` among the
    /// finishers at `finish_time_s` on the race clock.
    Finished {
        kart: usize,
        position: u32,
        finish_time_s: f32,
    },
    /// The kart was put back on the track at its last checkpoint.
    Respawned { kart: usize },
    /// The kart picked up `item_box`, an index into the track's item boxes, and
    /// rolled `item` into its slot.
    ItemPicked {
        kart: usize,
        item: Item,
        item_box: usize,
    },
    /// The kart fired the item it held; its slot is empty again.
    ItemFired { kart: usize, item: Item },
    /// The kart hit a hazard `by` fired, or was hit by a projectile `by`
    /// fired, and is spun out: stopped, with its inputs ignored, for the
    /// tuned time.
    SpunOut { kart: usize, by: usize },
}

/// Where the race is. Every value here is read from `step`, never from a
/// clock outside the sim, so a replay carries the whole lifecycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// The karts are held on the grid; Go comes when `remaining_s` runs out.
    /// The throttle hold ending at Go is the rocket start: a boost or a
    /// stall by its length.
    Countdown { remaining_s: f32 },
    /// The laps are being raced; `elapsed_s` is the race clock, which
    /// finish times are read against. Once the first kart finishes
    /// `grace_left_s` counts down the tuned grace, and the race ends when
    /// it runs out or every kart has finished, whichever comes first.
    Racing {
        elapsed_s: f32,
        grace_left_s: Option<f32>,
    },
    /// Over: the standings are the results and `step` changes nothing.
    Finished,
}

impl Phase {
    /// The phase as one byte in lifecycle order, for the hash and the bridge.
    pub fn code(self) -> u8 {
        match self {
            Self::Countdown { .. } => 0,
            Self::Racing { .. } => 1,
            Self::Finished => 2,
        }
    }
}

/// What a kart's rocket start comes to (GLOSSARY): during the countdown the
/// tier the throttle hold so far has reached, so the tiers come as the hold
/// lengthens and a hold that has tipped into a stall reads as one; from Go
/// the verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// No hold, or one under the weak edge: a plain start.
    Clean,
    /// A hold from the weak edge: the weak start boost.
    WeakBoost,
    /// A hold from the full edge: the full start boost.
    FullBoost,
    /// A hold past the stall edge: dead to every control for the tuned time.
    Stall,
}

impl Start {
    /// One byte in tier order, for the hash and the bridge.
    pub fn code(self) -> u8 {
        match self {
            Self::Clean => 0,
            Self::WeakBoost => 1,
            Self::FullBoost => 2,
            Self::Stall => 3,
        }
    }

    /// The tier a hold of `hold_s` earns. The hold is read off the
    /// countdown's clock, which runs down a tick of `dt` at a time, so each
    /// edge is read to the nearest half tick: a hold that began as the 2
    /// appeared is the full boost, not the stall.
    fn for_hold(hold_s: f32, dt: f32) -> Self {
        let half_tick = dt / 2.0;
        if hold_s > START_STALL_HOLD_S + half_tick {
            Self::Stall
        } else if hold_s >= START_FULL_BOOST_HOLD_S - half_tick {
            Self::FullBoost
        } else if hold_s >= START_WEAK_BOOST_HOLD_S - half_tick {
            Self::WeakBoost
        } else {
            Self::Clean
        }
    }
}

/// Where a kart stands in the race: what the Instrument panel shows and
/// what race position is ordered by. One per kart from [`State::standings`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    /// Race position, 1 for the leader. Finishers first in finishing order,
    /// then the rest by laps completed, then checkpoint, then spline
    /// progress; a dead heat keeps grid order.
    pub position: u32,
    /// The race clock as the kart completed the race's laps; `None` while
    /// it is still racing. Final once set: nothing after the line moves a
    /// finisher's place.
    pub finish_time_s: Option<f32>,
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
    /// The first this many `karts` are humans'; fixed for the race, though
    /// a bot may drive one for a while (see [`State::hand_to_bot`]).
    humans: usize,
    physics: Physics,
    phase: Phase,
    /// What the last tick changed that the players are told about. Not
    /// hashed: nothing here influences a future tick.
    events: Vec<RaceEvent>,
    /// Seconds until each of the track's item boxes is back, in the
    /// track's order; zero or less while the box is there to pick up.
    item_box_respawn_s: Vec<f32>,
    /// The hazards on the track, in the order they were fired.
    hazards: Vec<PlacedHazard>,
    /// The projectiles in flight, in the order they were fired.
    projectiles: Vec<Projectile>,
}

#[derive(Clone)]
struct Kart {
    body: RigidBodyHandle,
    collider: ColliderHandle,
    /// The bot driving this kart, with what it remembers; `None` while its
    /// human drives it.
    bot: Option<bot::Memory>,
    /// The unit direction the handling model sent the kart this tick: its
    /// facing, turned by the slip angle in a drift. What a wall's hold
    /// measures the surviving speed along.
    travel: Vec3,
    /// Unit normals, pointing at the kart, of the walls it ended the last
    /// tick against. The next tick's handling sends no velocity into them.
    walls: Vec<Vec3>,
    /// Velocity away from the karts it has touched, metres per second
    /// along the ground, added to the handling's each tick and decaying
    /// over the tuned time. Zero while nothing has bumped it lately.
    bump: Vec3,
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
    /// the start line, so the first lap begins at Go.
    checkpoint: usize,
    spline_progress: f32,
    lap_time_s: f32,
    best_lap_s: Option<f32>,
    /// The count the throttle hold under way began at, read before that
    /// tick's clock ran down; `None` while the throttle is up. The hold's
    /// length is this less the count now.
    start_hold_from_s: Option<f32>,
    /// The tier the hold so far has reached, as of the last countdown tick;
    /// from Go, the verdict, applied there.
    start: Start,
    /// The count a bot kart that rolled the boost presses the throttle at.
    /// Zero for a bot that rolled a clean start and for a human's kart.
    bot_start_hold_s: f32,
    /// Seconds of stall left after Go; no control does anything until it
    /// runs out. Zero or less when not stalled.
    stall_left_s: f32,
    /// The race clock at the finish; `None` while the kart is racing. A
    /// finished kart counts no more laps and its lap clock stops.
    finish_time_s: Option<f32>,
    /// The item in the kart's one slot. While it holds one it drives over
    /// item boxes without taking them.
    held_item: Option<Item>,
    /// Whether the item button was down last tick, so a held button fires
    /// once, as it goes down, and holding it through a pickup fires nothing.
    item_was_held: bool,
    /// Seconds of spin-out left after a hit; the kart is stopped and dead to
    /// every control until it runs out. Zero or less when not spun out.
    spin_out_left_s: f32,
}

impl Kart {
    fn finished(&self) -> bool {
        self.finish_time_s.is_some()
    }

    fn airborne(&self) -> bool {
        self.airtime_left_s > 0.0
    }

    fn spun_out(&self) -> bool {
        self.spin_out_left_s > 0.0
    }

    /// Dead to every control: stalled at Go or spun out by an item.
    fn ignores_inputs(&self) -> bool {
        self.stall_left_s > 0.0 || self.spun_out()
    }
}

/// Marks a collider as a kart's, where a track collider carries its
/// surface's code; no surface code reaches this.
const KART_COLLIDER: u128 = u128::MAX;

/// The box a kart collides with. Its length runs along -Z, its facing.
fn kart_collider() -> Collider {
    ColliderBuilder::round_cuboid(
        KART_HALF_WIDTH_M - KART_EDGE_RADIUS_M,
        KART_HALF_HEIGHT_M - KART_EDGE_RADIUS_M,
        KART_HALF_LENGTH_M - KART_EDGE_RADIUS_M,
        KART_EDGE_RADIUS_M,
    )
    .friction(0.0)
    .restitution(0.0)
    .user_data(KART_COLLIDER)
    .build()
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
    BOT_START_BOOST_CHANCE, BOT_START_HOLD_S, COUNTDOWN_S, RACE_END_GRACE_S, START_BOOST_FACTOR,
    START_BOOST_S, START_FULL_BOOST_HOLD_S, START_STALL_HOLD_S, START_STALL_S, START_THROTTLE_HELD,
    START_WEAK_BOOST_FACTOR, START_WEAK_BOOST_HOLD_S, START_WEAK_BOOST_S,
};
use tuning::{
    DRIFT_ENTRY_SPEED_FRACTION, GRAVITY_M_PER_S2, HOP_AIRTIME_S, KART_EDGE_RADIUS_M,
    KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M, LAPS_PER_RACE,
    MINI_TURBO_BOOST_FACTOR, OFF_TRACK_SPEED_FACTOR, TOP_SPEED_M_PER_S,
};
use tuning::{
    HAZARD_DROP_DISTANCE_M, HAZARD_LIFETIME_S, HAZARD_RADIUS_M, ITEM_BOOST_FACTOR, ITEM_BOOST_S,
    ITEM_BOX_HALF_LENGTH_M, ITEM_BOX_HALF_WIDTH_M, ITEM_BOX_RESPAWN_S, ITEM_ROLL_PERCENT,
    ITEM_ROLL_WEIGHTS, SPIN_OUT_S,
};
use tuning::{
    HOMING_LOCK_RANGE_M, HOMING_LOOKAHEAD_M, HOMING_RADIUS_M, HOMING_SPEED_FACTOR,
    HOMING_WITHOUT_TARGET_IS_WASTED,
};
use tuning::{
    KART_BUMP_DECAY_S, KART_BUMP_MIN_M_PER_S, KART_BUMP_RESTITUTION, KART_BUMP_SPEED_LOSS,
};

// The sim builds only the wasted fire for a projectile with no kart ahead
// (the decision is recorded with the constant); flipping it needs the
// circling projectile written first.
const _: () = assert!(HOMING_WITHOUT_TARGET_IS_WASTED);

/// A surface query starts this far above the asked point, so a point lying
/// exactly on a surface still finds it despite rounding.
const SURFACE_PROBE_RISE_M: f32 = 0.1;

/// How far ahead of a kart Rapier looks for contacts: past the farthest a
/// kart moves in one tick at the biggest boost, so no wall is entered
/// before it is seen.
const WALL_PREDICTION_M: f32 =
    2.0 * BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S / tuning::TICKS_PER_SECOND as f32;

/// Stands in for a missing best lap, finish time or grace in the hash; none
/// of them is ever negative.
const NO_TIME: f32 = -1.0;

/// A bump below this is spent: dropped to zero rather than decayed on
/// without end, so a settled kart hashes the same wherever it was bumped.
const BUMP_SPENT_M_PER_S: f32 = 0.01;

impl State {
    /// A fresh race on `track` with `kart_count` human karts on its spawn
    /// grid, in slot order, at the top of its countdown. `kart_count` must
    /// not exceed the track's spawn slots.
    pub fn new(seed: u64, kart_count: usize, track: Track) -> Self {
        Self::with_bots(seed, kart_count, 0, track)
    }

    /// A fresh race on `track` with `humans` karts on the front of its spawn
    /// grid and `bots` karts on the slots behind them, at the top of its
    /// countdown. Together they must not exceed the track's spawn slots.
    pub fn with_bots(seed: u64, humans: usize, bots: usize, track: Track) -> Self {
        let kart_count = humans + bots;
        assert!(
            kart_count <= track.spawn_grid().len(),
            "{} karts but the track has {} spawn slots",
            kart_count,
            track.spawn_grid().len()
        );
        Self::on_spawn_slots(seed, humans, 0..kart_count, track)
    }

    /// A fresh race with one kart on each of `slots` of the spawn grid, in
    /// order, the first `humans` of them human, at the top of its countdown.
    /// Each bot kart's rocket start is rolled here, in slot order, so the
    /// seed decides it.
    fn on_spawn_slots(seed: u64, humans: usize, slots: Range<usize>, track: Track) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
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

        let karts = track.spawn_grid()[slots]
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let body = RigidBodyBuilder::dynamic()
                    .translation(Vec3::from(slot.position) + Vec3::Y * KART_HALF_HEIGHT_M)
                    .rotation(Vec3::Y * slot.yaw)
                    // Steering is a yaw we set each tick, the one thing that
                    // turns a kart: a contact never tips, rolls or yaws it.
                    // Locked axes still take the velocity we set.
                    .lock_rotations()
                    // Contacts are predicted this far ahead, so a kart
                    // meets a wall where it stands instead of inside it.
                    .soft_ccd_prediction(WALL_PREDICTION_M)
                    .build();
                let body = physics.bodies.insert(body);
                let collider = physics.colliders.insert_with_parent(
                    kart_collider(),
                    body,
                    &mut physics.bodies,
                );
                let bot_start_hold_s = if index >= humans && rolls_the_start_boost(&mut rng) {
                    BOT_START_HOLD_S
                } else {
                    0.0
                };
                Kart {
                    body,
                    collider,
                    bot: (index >= humans).then_some(bot::Memory::default()),
                    travel: Vec3::NEG_Z,
                    walls: Vec::new(),
                    bump: Vec3::ZERO,
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
                    start_hold_from_s: None,
                    start: Start::Clean,
                    bot_start_hold_s,
                    stall_left_s: 0.0,
                    finish_time_s: None,
                    held_item: None,
                    item_was_held: false,
                    spin_out_left_s: 0.0,
                }
            })
            .collect();
        physics.index_colliders();

        Self {
            rng,
            item_box_respawn_s: vec![0.0; track.item_boxes().len()],
            hazards: Vec::new(),
            projectiles: Vec::new(),
            track,
            karts,
            humans,
            physics,
            phase: Phase::Countdown {
                remaining_s: COUNTDOWN_S,
            },
            events: Vec::new(),
        }
    }

    pub fn track(&self) -> &Track {
        &self.track
    }

    /// Where the race is, as of the last tick.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// What the last tick changed that the players are told about, in the
    /// order it happened; empty after a tick that changed nothing of the
    /// kind. A finish follows the completion of the kart's last lap.
    pub fn events(&self) -> &[RaceEvent] {
        &self.events
    }

    /// Karts driven from `step`'s inputs: the first this many grid slots.
    pub fn human_count(&self) -> usize {
        self.humans
    }

    /// Whether the kart in `slot` is driven by a bot.
    pub fn is_bot(&self, slot: usize) -> bool {
        self.karts[slot].bot.is_some()
    }

    /// Whether each of the track's item boxes, in the track's order, is
    /// there to pick up as of the last tick; a picked-up box is back the tuned
    /// time after it was taken.
    pub fn item_boxes_present(&self) -> Vec<bool> {
        self.item_box_respawn_s
            .iter()
            .map(|&respawn_s| respawn_s <= 0.0)
            .collect()
    }

    /// The hazards on the track as of the last tick, in the order they were
    /// fired.
    pub fn hazards(&self) -> Vec<Hazard> {
        self.hazards.iter().map(|placed| placed.hazard).collect()
    }

    /// The projectiles in flight as of the last tick, in the order they
    /// were fired.
    pub fn projectiles(&self) -> &[Projectile] {
        &self.projectiles
    }

    /// A bot drives the human's kart in `slot` from the next tick, with no
    /// memory of its own yet; `step` still takes the kart's input and
    /// ignores it. The server does this while it has lost the human.
    /// `slot` must be a human's.
    pub fn hand_to_bot(&mut self, slot: usize) {
        assert!(slot < self.humans, "kart {slot} is not a human's");
        let kart = &mut self.karts[slot];
        if kart.bot.is_none() {
            kart.bot = Some(bot::Memory::default());
        }
    }

    /// The human drives their kart in `slot` again from the next tick.
    /// `slot` must be a human's.
    pub fn hand_back(&mut self, slot: usize) {
        assert!(slot < self.humans, "kart {slot} is not a human's");
        self.karts[slot].bot = None;
    }

    /// The rubber band on a kart's top speed for the coming tick: a bot the
    /// tuned range or more ahead of the nearest human along the spline is
    /// held to the tuned minimum, one that far behind is let up to the tuned
    /// maximum, and the scale runs linearly between. A human's kart, even
    /// one a bot drives for now, or a bot in a race with no humans, is
    /// scaled by one.
    pub fn rubber_band(&self, slot: usize) -> f32 {
        if slot < self.humans || self.humans == 0 {
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

    /// A hash of everything that influences future ticks: the phase and its
    /// clocks, the RNG, the item boxes' respawn clocks, the hazards and
    /// projectiles and, per kart, its speed, its item and its body's
    /// position, heading, velocities and sleep state, including the timer
    /// that puts a resting body to sleep.
    /// Bit-identical on every platform for bit-identical states, which is
    /// what the replay fixtures check (ADR 0002). A bot's memory is hashed
    /// with its kart, so a human's kart a bot drives hashes apart from one
    /// its human does. Three things are left out on purpose. The track and
    /// the split between human and bot slots never change after `new`, so
    /// they are the replay's `track` and `bots` fields instead. Rapier's
    /// contact manifolds and warm-start impulses are not hashed; a
    /// difference there reaches a body's velocity on the next tick and is
    /// caught then.
    pub fn hash(&self) -> u64 {
        let mut hasher = hash::Fnv1a::new();
        hasher.write_u8(self.phase.code());
        match self.phase {
            Phase::Countdown { remaining_s } => hasher.write_f32(remaining_s),
            Phase::Racing {
                elapsed_s,
                grace_left_s,
            } => {
                hasher.write_f32(elapsed_s);
                hasher.write_f32(grace_left_s.unwrap_or(NO_TIME));
            }
            Phase::Finished => {}
        }
        hasher.write(&self.rng.get_seed());
        hasher.write_u64(self.rng.get_stream());
        hasher.write_u128(self.rng.get_word_pos());
        hasher.write_f32s(self.item_box_respawn_s.iter().copied());
        hasher.write_u64(self.hazards.len() as u64);
        for placed in &self.hazards {
            hasher.write_f32s(placed.hazard.position);
            hasher.write_u64(placed.hazard.owner as u64);
            hasher.write_u8(u8::from(placed.hazard.held));
            hasher.write_f32(placed.lifetime_left_s);
            hasher.write_u8(u8::from(placed.clear_of_owner));
        }
        hasher.write_u64(self.projectiles.len() as u64);
        for projectile in &self.projectiles {
            hasher.write_f32s(projectile.position);
            hasher.write_u64(projectile.owner as u64);
            hasher.write_u64(projectile.target as u64);
        }
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
            hasher.write_f32s(kart.bump.to_array());
            hasher.write_f32(kart.out_of_world_s);
            hasher.write_f32(kart.wrong_way_s);
            hasher.write_u32(kart.laps_completed);
            hasher.write_u64(kart.checkpoint as u64);
            hasher.write_f32(kart.spline_progress);
            hasher.write_f32(kart.lap_time_s);
            hasher.write_f32(kart.best_lap_s.unwrap_or(NO_TIME));
            hasher.write_f32(kart.start_hold_from_s.unwrap_or(NO_TIME));
            hasher.write_u8(kart.start.code());
            hasher.write_f32(kart.bot_start_hold_s);
            hasher.write_f32(kart.stall_left_s);
            hasher.write_f32(kart.finish_time_s.unwrap_or(NO_TIME));
            hasher.write_u8(kart.held_item.map_or(0, Item::code));
            hasher.write_u8(u8::from(kart.item_was_held));
            hasher.write_f32(kart.spin_out_left_s);
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
        // Stable, so karts level on every key keep grid order. A finish time
        // sorts a finisher ahead of everyone still racing and among the
        // other finishers.
        order.sort_by(|&a, &b| {
            let (a, b) = (&self.karts[a], &self.karts[b]);
            let finish = |kart: &Kart| kart.finish_time_s.unwrap_or(f32::INFINITY);
            finish(a)
                .total_cmp(&finish(b))
                .then(b.laps_completed.cmp(&a.laps_completed))
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
                finish_time_s: kart.finish_time_s,
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
                    velocity: body.linvel().to_array(),
                    drift: kart.drift,
                    held_item: kart.held_item,
                    spin_out_s: kart.spun_out().then_some(kart.spin_out_left_s),
                    airborne: kart.airborne(),
                    boost_s: (kart.boost_left_s > 0.0).then_some(kart.boost_left_s),
                    start: kart.start,
                    stall_s: (kart.stall_left_s > 0.0).then_some(kart.stall_left_s),
                }
            })
            .collect()
    }

    /// Take the item box `item_box` into the slot of the kart in `slot`,
    /// rolling the item by the kart's race position as of `standings`, and
    /// start the box's respawn clock.
    fn pick_up(&mut self, slot: usize, item_box: usize, standings: &[Standing]) {
        // The draw scaled down to the table's range, so the roll is as even
        // as the draw.
        let roll = ((u64::from(self.rng.next_u32()) * u64::from(ITEM_ROLL_PERCENT)) >> 32) as u32;
        let item = Item::rolled(standings[slot].position, roll);
        self.karts[slot].held_item = Some(item);
        self.item_box_respawn_s[item_box] = ITEM_BOX_RESPAWN_S;
        self.events.push(RaceEvent::ItemPicked {
            kart: slot,
            item,
            item_box,
        });
    }
}

/// Advance the race by one tick of `dt` seconds. `inputs` holds one entry per
/// human kart in grid order; the caller must supply exactly as many as there
/// are humans. The bots' inputs come from [`bot::drive`], given the readouts
/// as of the last tick, and so does that of a human's kart a bot drives for
/// now, whose entry in `inputs` is ignored.
///
/// During the countdown nothing moves and only the throttle is read, for
/// the rocket start: the count the hold under way began at, Go's tick
/// included, decides the start. A bot kart that rolled the boost at spawn
/// holds from `BOT_START_HOLD_S`; a human's kart a bot drives for now
/// holds nothing, so it starts clean. Once the race has finished the state
/// is returned as it was.
pub fn step(mut state: State, inputs: &[KartInput], dt: f32) -> State {
    assert_eq!(inputs.len(), state.humans, "one input per human kart");
    state.events.clear();
    let (elapsed_s, grace_left_s) = match state.phase {
        Phase::Finished => return state,
        Phase::Countdown { remaining_s } => {
            let humans = state.humans;
            for (slot, kart) in state.karts.iter_mut().enumerate() {
                let held = if slot < humans {
                    kart.bot.is_none() && inputs[slot].throttle > START_THROTTLE_HELD
                } else {
                    remaining_s <= kart.bot_start_hold_s
                };
                kart.start_hold_from_s = match kart.start_hold_from_s {
                    Some(from) if held => Some(from),
                    None if held => Some(remaining_s),
                    _ => None,
                };
            }
            state.phase = clock_after(state.phase, dt);
            let count_s = match state.phase {
                Phase::Countdown { remaining_s } => remaining_s,
                _ => 0.0,
            };
            for kart in &mut state.karts {
                let hold_s = kart.start_hold_from_s.map_or(0.0, |from| from - count_s);
                kart.start = Start::for_hold(hold_s, dt);
            }
            // Go: the verdict takes effect.
            if let Phase::Racing { .. } = state.phase {
                for kart in &mut state.karts {
                    apply_start(kart, kart.start);
                }
            }
            return state;
        }
        Phase::Racing {
            elapsed_s,
            grace_left_s,
        } => (elapsed_s + dt, grace_left_s),
    };
    // Skip building the readouts when no bot needs them.
    let inputs: Vec<KartInput> = if state.karts.iter().all(|kart| kart.bot.is_none()) {
        inputs.to_vec()
    } else {
        let transforms = state.kart_transforms();
        let standings = state.standings();
        let track = &state.track;
        state
            .karts
            .iter_mut()
            .enumerate()
            .map(|(slot, kart)| match &mut kart.bot {
                Some(memory) => bot::drive(track, &transforms, &standings, slot, memory, dt),
                None => inputs[slot],
            })
            .collect()
    };
    // Race positions as of the last tick: what a projectile picks its
    // target by and a pickup rolls by.
    let standings = state.standings();
    // Fires come before pickups, so a press on the tick a box is picked up
    // fires what the kart held, never what it has just rolled. A stalled or
    // spun-out kart's press is as dead as its other controls.
    for (slot, input) in inputs.iter().enumerate() {
        let kart = &mut state.karts[slot];
        let pressed = input.use_item || (input.hold_item && !kart.item_was_held);
        kart.item_was_held = input.hold_item;
        if kart.ignores_inputs() || !pressed {
            continue;
        }
        if let Some(item) = kart.held_item.take() {
            match item {
                Item::Boost => {
                    kart.boost_left_s = ITEM_BOOST_S;
                    kart.boost_factor = ITEM_BOOST_FACTOR;
                }
                Item::Hazard => {
                    let body = &state.physics.bodies[kart.body];
                    state.hazards.push(PlacedHazard {
                        hazard: Hazard {
                            position: behind(body.translation(), body.rotation()),
                            owner: slot,
                            held: true,
                        },
                        lifetime_left_s: HAZARD_LIFETIME_S,
                        clear_of_owner: false,
                    });
                }
                // Fired with no kart ahead, the leader's projectile is
                // wasted: the fire is reported and nothing flies.
                Item::Homing => {
                    if let Some(target) = kart_ahead(&standings, slot) {
                        let here = state.physics.bodies[kart.body].translation();
                        state.projectiles.push(Projectile {
                            position: at_centreline_height(&state.track, here),
                            owner: slot,
                            target,
                        });
                    }
                }
            }
            state.events.push(RaceEvent::ItemFired { kart: slot, item });
        }
    }
    // Picked-up boxes come back on their clocks; a kart standing on one
    // picks it up the tick it is back. A kart holding an item drives over boxes.
    for respawn_s in &mut state.item_box_respawn_s {
        if *respawn_s > 0.0 {
            *respawn_s -= dt;
        }
    }
    for slot in 0..state.karts.len() {
        if state.karts[slot].held_item.is_some() {
            continue;
        }
        let position = state.physics.bodies[state.karts[slot].body]
            .translation()
            .to_array();
        let picked = state
            .track
            .item_boxes()
            .iter()
            .zip(&state.item_box_respawn_s)
            .position(|(item_box, &respawn_s)| {
                respawn_s <= 0.0
                    && item_box.covers(position, ITEM_BOX_HALF_LENGTH_M, ITEM_BOX_HALF_WIDTH_M)
            });
        if let Some(item_box) = picked {
            state.pick_up(slot, item_box, &standings);
        }
    }
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
    // Whether each kart holds the item button this tick, as the controls
    // reach the handling: a kart dead to its controls holds nothing.
    let mut holds = vec![false; state.karts.len()];
    for ((((kart, input), surface), top_speed), hold) in state
        .karts
        .iter_mut()
        .zip(&inputs)
        .zip(surfaces)
        .zip(top_speeds)
        .zip(&mut holds)
    {
        // A stalled or spun-out kart sits dead to every control until its
        // time runs out.
        let input = if kart.ignores_inputs() {
            kart.stall_left_s -= dt;
            kart.spin_out_left_s -= dt;
            &KartInput::default()
        } else {
            input
        };
        *hold = input.hold_item;
        let body = &mut state.physics.bodies[kart.body];
        // A spun-out kart stands where it is, whatever runs into it; the
        // tick its spin-out runs out it moves again.
        body.lock_translations(kart.spun_out(), true);
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
        // A spin-out stops the kart whatever else it had going, a boost
        // included: a prediction that learns of a hit from a snapshot still
        // holds a boost the snapshot does not carry.
        if kart.spun_out() {
            kart.speed = 0.0;
            kart.boost_left_s = 0.0;
            kart.bump = Vec3::ZERO;
        }
        let (steer, slip) = match kart.drift {
            Some(drift) => (
                drift_steer(drift.direction, input.steer),
                drift_slip(drift.direction),
            ),
            None => (input.steer.clamp(-1.0, 1.0), 0.0),
        };
        let forward = *body.rotation() * Rotation::from_rotation_y(slip) * Vec3::NEG_Z;
        kart.bump = decayed_bump(kart.bump, dt);
        // A wall the kart is against takes the velocity into it, speed and
        // bump alike, before the solver sees either; otherwise the kart
        // would press into the wall a little every tick. The bump stays
        // apart from the speed, so it is never counted against it twice.
        let mut velocity = forward * kart.speed + kart.bump;
        if !kart.walls.is_empty() {
            velocity = stopped_by_walls(velocity, &kart.walls);
            kart.bump = stopped_by_walls(kart.bump, &kart.walls);
            kart.speed = forward.dot(velocity - kart.bump);
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
    // The velocities the handling sent the karts off with, and the bumps
    // in them, before the solver's contacts bent them: what a touch
    // between karts is judged by, and what a wall's hold leaves the speed
    // from.
    let sent_velocities: Vec<Vec3> = state
        .karts
        .iter()
        .map(|kart| state.physics.bodies[kart.body].linvel())
        .collect();
    let sent_bumps: Vec<Vec3> = state.karts.iter().map(|kart| kart.bump).collect();
    state.physics.step(dt);
    for index in 0..state.karts.len() {
        bump_off_karts(
            &mut state.karts,
            index,
            &state.physics,
            &sent_velocities,
            dt,
        );
    }
    let mut finished_this_tick = Vec::new();
    for (index, (kart, mut before)) in state.karts.iter_mut().zip(before).enumerate() {
        held_by_walls(
            kart,
            &mut state.physics,
            sent_velocities[index],
            sent_bumps[index],
            dt,
        );
        let mut after = state.physics.bodies[kart.body].translation().to_array();
        if state.physics.surface_at(after) == Surface::OutOfWorld {
            kart.out_of_world_s += dt;
            if kart.out_of_world_s >= RESPAWN_DELAY_S {
                after = respawn(kart, &state.track, &mut state.physics);
                state.events.push(RaceEvent::Respawned { kart: index });
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
        if !kart.finished() {
            kart.lap_time_s += dt;
            if let Some(lap_time_s) =
                cross_checkpoint(kart, &state.track, before, after, dt, elapsed_s)
            {
                state.events.push(RaceEvent::LapCompleted {
                    kart: index,
                    lap: kart.laps_completed,
                    lap_time_s,
                });
                if let Some(finish_time_s) = kart.finish_time_s {
                    finished_this_tick.push((index, finish_time_s));
                }
            }
        }
    }
    // A held hazard trails its owner to where the owner ended the tick;
    // letting go of the button, or losing the controls, leaves it there.
    for placed in &mut state.hazards {
        if !placed.hazard.held {
            continue;
        }
        let owner = &state.karts[placed.hazard.owner];
        if holds[placed.hazard.owner] {
            let body = &state.physics.bodies[owner.body];
            placed.hazard.position = behind(body.translation(), body.rotation());
        } else {
            placed.hazard.held = false;
        }
    }
    // A dropped hazard ages away; one still lying there spins out the first
    // kart to reach it, its owner included once the owner has got clear of
    // it, and is used up. A kart already spun out hits nothing more.
    let mut hazards = std::mem::take(&mut state.hazards);
    hazards.retain_mut(|placed| {
        if !placed.hazard.held {
            placed.lifetime_left_s -= dt;
            if placed.lifetime_left_s <= 0.0 {
                return false;
            }
        }
        for (slot, kart) in state.karts.iter_mut().enumerate() {
            let body = &mut state.physics.bodies[kart.body];
            let reached = reaches(
                body.translation(),
                body.rotation(),
                placed.hazard.position,
                HAZARD_RADIUS_M,
            );
            if slot == placed.hazard.owner && !placed.clear_of_owner {
                placed.clear_of_owner = !reached;
                continue;
            }
            if !reached || kart.spun_out() {
                continue;
            }
            spin_out(kart, body);
            state.events.push(RaceEvent::SpunOut {
                kart: slot,
                by: placed.hazard.owner,
            });
            return false;
        }
        true
    });
    state.hazards = hazards;
    // A projectile flies after its target and is used up on reaching the
    // kart: on a hazard the kart trails, which goes with it, else on the
    // kart itself, which spins out unless it already is.
    let mut projectiles = std::mem::take(&mut state.projectiles);
    projectiles.retain_mut(|projectile| {
        let target = &mut state.karts[projectile.target];
        let body = &mut state.physics.bodies[target.body];
        fly(projectile, &state.track, body.translation(), dt);
        if !reaches(
            body.translation(),
            body.rotation(),
            projectile.position,
            HOMING_RADIUS_M,
        ) {
            return true;
        }
        let shield = state
            .hazards
            .iter()
            .position(|placed| placed.hazard.held && placed.hazard.owner == projectile.target);
        match shield {
            Some(index) => {
                state.hazards.remove(index);
            }
            None if !target.spun_out() => {
                spin_out(target, body);
                state.events.push(RaceEvent::SpunOut {
                    kart: projectile.target,
                    by: projectile.owner,
                });
            }
            None => {}
        }
        false
    });
    state.projectiles = projectiles;
    // Places are read once every kart has moved, so two karts finishing on
    // the same tick are placed by their finish times, as the standings are.
    if !finished_this_tick.is_empty() {
        let standings = state.standings();
        finished_this_tick.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (index, finish_time_s) in finished_this_tick {
            state.events.push(RaceEvent::Finished {
                kart: index,
                position: standings[index].position,
                finish_time_s,
            });
        }
    }
    state.phase = next_phase(&state.karts, elapsed_s, grace_left_s, dt);
    state
}

/// Whether a bot kart rolls the full boost, drawn on the race's RNG at the
/// tuned chance.
fn rolls_the_start_boost(rng: &mut ChaCha8Rng) -> bool {
    (f64::from(rng.next_u32()) / f64::from(u32::MAX)) < f64::from(BOT_START_BOOST_CHANCE)
}

/// Give `kart` what `start` earns at Go: a boost for its tuned time, or the
/// stall.
fn apply_start(kart: &mut Kart, start: Start) {
    match start {
        Start::Clean => {}
        Start::WeakBoost => {
            kart.boost_left_s = START_WEAK_BOOST_S;
            kart.boost_factor = START_WEAK_BOOST_FACTOR;
        }
        Start::FullBoost => {
            kart.boost_left_s = START_BOOST_S;
            kart.boost_factor = START_BOOST_FACTOR;
        }
        Start::Stall => kart.stall_left_s = START_STALL_S,
    }
}

/// The race clock one tick of `dt` on: the countdown runs down to Go, and
/// from Go the race clock runs, starting with the tick after. The end of
/// the race is not the clock's to call; `next_phase` decides it.
fn clock_after(phase: Phase, dt: f32) -> Phase {
    match phase {
        Phase::Countdown { remaining_s } => {
            let remaining_s = remaining_s - dt;
            if remaining_s > 0.0 {
                Phase::Countdown { remaining_s }
            } else {
                Phase::Racing {
                    elapsed_s: 0.0,
                    grace_left_s: None,
                }
            }
        }
        Phase::Racing {
            elapsed_s,
            grace_left_s,
        } => Phase::Racing {
            elapsed_s: elapsed_s + dt,
            grace_left_s,
        },
        Phase::Finished => Phase::Finished,
    }
}

/// The phase after a racing tick that ended at `elapsed_s` with
/// `grace_left_s` as it stood before the tick: the grace starts the tick
/// the first kart finishes, and the race ends when the grace runs out or
/// every kart has finished.
fn next_phase(karts: &[Kart], elapsed_s: f32, grace_left_s: Option<f32>, dt: f32) -> Phase {
    if karts.iter().all(Kart::finished) {
        return Phase::Finished;
    }
    let grace_left_s = match grace_left_s {
        Some(left) => Some(left - dt),
        None if karts.iter().any(Kart::finished) => Some(RACE_END_GRACE_S),
        None => None,
    };
    if grace_left_s.is_some_and(|left| left <= 0.0) {
        return Phase::Finished;
    }
    Phase::Racing {
        elapsed_s,
        grace_left_s,
    }
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

/// The wall holds the kart, with no bounce and no bump. Every wall the
/// kart ends the tick against is remembered for the next tick's handling,
/// and a wall met this tick takes the velocity into it there and then: the
/// velocity along the wall is kept and the kart's speed becomes what is
/// left along its travel once the bump that was in the velocity, the
/// `sent_bump`, is taken off. A kart facing straight at a wall stops dead;
/// one scraping along at an angle loses the share of its speed that
/// pointed into the wall each tick, so it slows until it steers away.
fn held_by_walls(
    kart: &mut Kart,
    physics: &mut Physics,
    sent_velocity: Vec3,
    sent_bump: Vec3,
    dt: f32,
) {
    let velocity = physics.bodies[kart.body].linvel();
    kart.walls.clear();
    for pair in physics.narrow_phase.contact_pairs_with(kart.collider) {
        let other = other_collider(pair, kart.collider);
        if physics.colliders[other].user_data != u128::from(Surface::Wall.code()) {
            continue;
        }
        for manifold in pair.manifolds() {
            let normal = normal_toward(pair, kart.collider, manifold);
            // Contact distances are from the start of the tick; the
            // velocity the kart set off with says whether it met the wall
            // this tick, before the solver held it short.
            let closing = sent_velocity.dot(normal) * dt;
            if manifold
                .points
                .iter()
                .any(|contact| contact.dist + closing <= WALL_TOUCH_M)
            {
                kart.walls.push(normal);
            }
        }
    }
    if kart.walls.is_empty() {
        return;
    }
    // The solver may already have taken the velocity into the wall; the
    // speed is what is left along the travel either way. A touch with a
    // kart this tick may have cost the speed more already: the lesser
    // stands, since neither ever adds to it.
    let stopped = stopped_by_walls(velocity, &kart.walls);
    if stopped != velocity {
        physics.bodies[kart.body].set_linvel(stopped, true);
    }
    let held = kart
        .travel
        .dot(stopped - stopped_by_walls(sent_bump, &kart.walls));
    if held.abs() < kart.speed.abs() {
        kart.speed = held;
    }
}

/// The collider `collider` is in contact with through `pair`.
fn other_collider(pair: &ContactPair, collider: ColliderHandle) -> ColliderHandle {
    if pair.collider1 == collider {
        pair.collider2
    } else {
        pair.collider1
    }
}

/// `manifold`'s contact normal pointing out of the other collider at
/// `collider`. Rapier's points from the pair's first collider to its
/// second, whichever `collider` is.
fn normal_toward(pair: &ContactPair, collider: ColliderHandle, manifold: &ContactManifold) -> Vec3 {
    if pair.collider1 == collider {
        -manifold.data.normal
    } else {
        manifold.data.normal
    }
}

/// The bump `dt` seconds on: a third of itself every `KART_BUMP_DECAY_S`,
/// and gone once it is too small to move the kart.
fn decayed_bump(bump: Vec3, dt: f32) -> Vec3 {
    let decayed = bump * libm::expf(-dt / KART_BUMP_DECAY_S);
    if decayed.length_squared() < BUMP_SPENT_M_PER_S * BUMP_SPENT_M_PER_S {
        Vec3::ZERO
    } else {
        decayed
    }
}

/// Karts bounce off each other: the kart in `index` that ended the tick
/// touching another takes a bump away from it, the least one plus the
/// tuned share of the speed the two closed at, judged by the velocities
/// the handling sent them off with; and when it was driving into the
/// other it loses the tuned share of that closing speed, scaled by how
/// square it hit, the way it was going, forward or in reverse. A bump
/// already under way is raised to the new one, never added to, so a touch
/// that lasts holds the karts apart without flinging them. A spun-out
/// kart takes no bump: it stands, and the other kart bounces off it.
/// A kart that is not one of `karts`, a kinematic stand-in for another
/// player's kart in a prediction, closes at the velocity the solver moved
/// it with. Each kart of a pair runs this for itself.
fn bump_off_karts(
    karts: &mut [Kart],
    index: usize,
    physics: &Physics,
    sent_velocities: &[Vec3],
    dt: f32,
) {
    // A spun-out kart stands where it is, whatever runs into it.
    if karts[index].spun_out() {
        return;
    }
    let collider = karts[index].collider;
    let mut bump = karts[index].bump;
    let mut speed = karts[index].speed;
    // The way the kart is going: its travel, or back along it in reverse.
    let heading = if speed < 0.0 {
        -karts[index].travel
    } else {
        karts[index].travel
    };
    for pair in physics.narrow_phase.contact_pairs_with(collider) {
        let other = other_collider(pair, collider);
        if physics.colliders[other].user_data != KART_COLLIDER {
            continue;
        }
        let other_body = physics.colliders[other]
            .parent()
            .expect("a kart collider has a body");
        let other_sent = karts
            .iter()
            .position(|kart| kart.body == other_body)
            .map_or(physics.bodies[other_body].linvel(), |slot| {
                sent_velocities[slot]
            });
        for manifold in pair.manifolds() {
            let normal = normal_toward(pair, collider, manifold);
            let normal = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
            if normal == Vec3::ZERO {
                continue;
            }
            // Contact distances are from the start of the tick; the
            // velocities the karts set off with say whether they met this
            // tick, before the solver held them apart.
            let closing_speed = (other_sent - sent_velocities[index]).dot(normal).max(0.0);
            if !manifold
                .points
                .iter()
                .any(|contact| contact.dist - closing_speed * dt <= WALL_TOUCH_M)
            {
                continue;
            }
            let wanted = KART_BUMP_MIN_M_PER_S + KART_BUMP_RESTITUTION * closing_speed / 2.0;
            let along = bump.dot(normal);
            if wanted > along {
                bump += normal * (wanted - along);
            }
            let square = (-heading.dot(normal)).max(0.0);
            let loss = KART_BUMP_SPEED_LOSS * square * closing_speed;
            speed = if speed < 0.0 {
                (speed + loss).min(0.0)
            } else {
                (speed - loss).max(0.0)
            };
        }
    }
    karts[index].bump = bump;
    karts[index].speed = speed;
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
    kart.bump = Vec3::ZERO;
    kart.out_of_world_s = 0.0;
    kart.wrong_way_s = 0.0;
    position.to_array()
}

/// Credit the kart with the checkpoint its move from `before` to `after`
/// crossed, when it is the next in order; a lap completes at the start
/// line after the last one, and the race's last lap finishes the kart. The
/// lap's time ends where the move met the line, so lap times and finish
/// times are finer than a tick, and the next lap's clock starts with the
/// rest of the tick. `elapsed_s` is the race clock at the end of the tick.
/// Returns the time of the lap the move completed, if it completed one.
fn cross_checkpoint(
    kart: &mut Kart,
    track: &Track,
    before: [f32; 3],
    after: [f32; 3],
    dt: f32,
    elapsed_s: f32,
) -> Option<f32> {
    let checkpoints = track.checkpoints();
    let next = (kart.checkpoint + 1) % checkpoints.len();
    let fraction = checkpoints[next].crossed_between(before, after)?;
    let mut completed = None;
    if next == 0 {
        let rest_of_tick_s = dt * (1.0 - fraction);
        let lap_time_s = kart.lap_time_s - rest_of_tick_s;
        kart.laps_completed += 1;
        kart.best_lap_s = Some(
            kart.best_lap_s
                .map_or(lap_time_s, |best| best.min(lap_time_s)),
        );
        if kart.laps_completed >= LAPS_PER_RACE {
            kart.finish_time_s = Some(elapsed_s - rest_of_tick_s);
            kart.lap_time_s = lap_time_s;
        } else {
            kart.lap_time_s = rest_of_tick_s;
        }
        completed = Some(lap_time_s);
    }
    kart.checkpoint = next;
    completed
}

/// Where a hazard dropped by a kart standing at `translation` facing
/// `rotation` lies: the tuned distance behind the kart, on the surface
/// under its centre.
fn behind(translation: Vec3, rotation: &Rotation) -> [f32; 3] {
    let rear = *rotation * Vec3::Z;
    (translation + rear * HAZARD_DROP_DISTANCE_M - Vec3::Y * KART_HALF_HEIGHT_M).to_array()
}

/// Whether a kart standing at `translation` facing `rotation` reaches a
/// hazard or a projectile centred at `point`: the point lies within
/// `radius` of the kart's collision box, seen from above.
fn reaches(translation: Vec3, rotation: &Rotation, point: [f32; 3], radius: f32) -> bool {
    let local = rotation.inverse() * (Vec3::from(point) - translation);
    let nearest = Vec3::new(
        local.x.clamp(-KART_HALF_WIDTH_M, KART_HALF_WIDTH_M),
        0.0,
        local.z.clamp(-KART_HALF_LENGTH_M, KART_HALF_LENGTH_M),
    );
    let (dx, dz) = (local.x - nearest.x, local.z - nearest.z);
    dx * dx + dz * dz <= radius * radius
}

/// The kart next ahead of the one in `slot` by race position, as
/// `standings` has it; `None` when that kart leads.
fn kart_ahead(standings: &[Standing], slot: usize) -> Option<usize> {
    standings
        .iter()
        .position(|standing| standing.position + 1 == standings[slot].position)
}

/// `point` brought to the height of the centreline nearest it, where a
/// projectile rides.
fn at_centreline_height(track: &Track, point: Vec3) -> [f32; 3] {
    let height = track.point_at(track.progress_at(point.to_array()))[1];
    [point.x, height, point.z]
}

/// Fly the projectile one tick of `dt` seconds at the tuned speed: along
/// the centreline, steering for the point the tuned lookahead ahead of
/// where it is, until its target at `target` is within the tuned lock
/// range, then straight at the target. Seen from above; the height is the
/// centreline's where it ends up.
fn fly(projectile: &mut Projectile, track: &Track, target: Vec3, dt: f32) {
    let here = Vec3::from(projectile.position);
    let to_target = Vec3::new(target.x - here.x, 0.0, target.z - here.z);
    let aim = if to_target.length() <= HOMING_LOCK_RANGE_M {
        to_target
    } else {
        let progress = track.progress_at(projectile.position);
        let ahead = track.point_at(progress + HOMING_LOOKAHEAD_M);
        Vec3::new(ahead[0] - here.x, 0.0, ahead[2] - here.z)
    };
    let reach = HOMING_SPEED_FACTOR * TOP_SPEED_M_PER_S * dt;
    let moved = if aim.length() <= reach {
        aim
    } else {
        aim.normalize() * reach
    };
    projectile.position = at_centreline_height(track, here + moved);
}

/// Stop the kart where it is and start its spin-out: a drift or a boost
/// ends, a hop lands without a drift, and no control reaches it and no
/// kart pushes it until the spin-out runs out.
fn spin_out(kart: &mut Kart, body: &mut RigidBody) {
    kart.spin_out_left_s = SPIN_OUT_S;
    kart.speed = 0.0;
    kart.drift = None;
    kart.hop_direction = None;
    kart.boost_left_s = 0.0;
    body.set_linvel(Vec3::ZERO, true);
    body.set_angvel(Vec3::ZERO, true);
    body.lock_translations(true, true);
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
