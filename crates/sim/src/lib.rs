//! The rules of the game, run by the browser (through `bridge`) and the
//! server. Must stay free of I/O, async runtimes and anything else from this
//! repo so both sides run identical rules.
//!
//! The public seam is [`step`]: feed a [`State`] one tick of [`KartInput`]
//! per kart and get the next state back. Everything a renderer needs is read
//! through [`State::kart_readouts`]. A race runs on a [`Track`] loaded from
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
//! a respawn, an item or a coin picked up, an item fired, a spin-out and
//! the coins it cost, is read from
//! [`State::events`] after that tick.

pub mod bot;
mod handling;
mod hash;
pub mod interpolation;
pub mod prediction;
pub mod replay;
pub mod track;
pub mod tuning;
mod wheel;

pub use interpolation::Interpolation;
pub use prediction::Prediction;
pub use replay::{Replay, ReplayError, Segment};
pub use track::{Checkpoint, Placement, Surface, SurfaceMesh, Track, TrackError, WheelCrossing};
pub use wheel::{wheel, Wheel};

use handling::{Freeze, Handling, Situation};

use rand_chacha::rand_core::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::Hasher;
use std::ops::Range;
use ts_rs::TS;

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

/// What the sim says of one kart at a tick (GLOSSARY: kart readout): what
/// the View, the Instrument panel and the bots read, what a snapshot carries
/// of the kart and what a prediction's kart adopts from one. Y is up and a
/// kart with identity rotation faces -Z, matching Three.js.
///
/// The transform, `position` and `rotation`, crosses the wasm seam as a
/// flat buffer (ADR 0003); the rest is serialised in the shape of the
/// generated TypeScript type (ADR 0026).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct KartReadout {
    /// Metres.
    #[serde(skip)]
    pub position: [f32; 3],
    /// A unit quaternion `[x, y, z, w]`.
    #[serde(skip)]
    pub rotation: [f32; 4],
    /// Metres per second along the kart's facing; negative when backing up.
    pub speed: f32,
    /// The body's velocity in metres per second: the speed along the
    /// travel, which a drift slips from the facing, and the vertical motion
    /// of a hop or a fall.
    pub velocity: [f32; 3],
    /// The bump the kart carries from a kart or tyre wall it touched, in
    /// metres per second, while it carries one: the part of `velocity`
    /// that is not its own driving, which decays away over the tuned time.
    /// A snapshot carries it so a prediction's kart adopts the server's
    /// bump on a rewind instead of keeping its own.
    pub bump: Option<[f32; 3]>,
    /// The drift the kart is in; `None` when it is not drifting.
    pub drift: Option<Drift>,
    /// The tier the drift has charged, the sparks' tell; `MiniTurbo::None`
    /// out of a drift.
    pub mini_turbo: MiniTurbo,
    /// The item in the kart's slot; `None` while the slot is empty.
    pub held_item: Option<Item>,
    /// The coins the kart carries, from 0 to a full purse; each raises its
    /// forward top speed by the tuned factor.
    pub coins: u32,
    /// Seconds of spin-out left, while the kart is spun out: stopped, with
    /// its inputs ignored. The View spins it for the duration.
    pub spin_out_s: Option<f32>,
    /// Whether the kart is in a hop's airtime; the hop's tell. Only the sim
    /// that steps the kart reports it: a snapshot does not carry it, so an
    /// interpolated kart is never airborne.
    pub airborne: bool,
    /// Seconds of boost left, while a mini-turbo, boost pad, item or start
    /// boost holds the kart over top speed; the release's tell.
    pub boost_s: Option<f32>,
    /// The tier the boost burns in, the flame's: orange for an orange
    /// release or a weak rocket start, red for every other boost;
    /// `MiniTurbo::None` while not boosting.
    pub boost_tier: MiniTurbo,
    /// The kart's rocket start: while the countdown runs the tier its hold
    /// so far has reached, the sparks' tell, and from Go the verdict.
    pub start: Start,
    /// Seconds of stall left, while the kart is stalled at Go: dead to
    /// every control, as in a spin-out. The sputter's tell.
    pub stall_s: Option<f32>,
    /// Race position from 1, as [`Standing::position`] orders it.
    pub race_position: u32,
    /// The lap the kart is on, from 1, as [`Standing::lap`] counts it.
    pub lap: u32,
    /// The last checkpoint crossed in order; 0 is the start line.
    pub checkpoint: usize,
    /// Metres along the centreline from the start line.
    pub spline_progress: f32,
    /// The wrong-way warning is up.
    pub wrong_way: bool,
}

impl KartReadout {
    /// The speed along the facing of a kart turned by `rotation` and moving
    /// at `velocity`: what a kart only a snapshot speaks for is reported as
    /// going.
    pub fn facing_speed(rotation: [f32; 4], velocity: [f32; 3]) -> f32 {
        Vec3::from(velocity).dot(Rotation::from_array(rotation) * Vec3::NEG_Z)
    }
}

/// The items a kart can hold: what an item box rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
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

/// A projectile as the rules keep it: what the View sees, plus its spline
/// progress, which its next reading is searched near.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FlyingProjectile {
    projectile: Projectile,
    progress: f32,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    pub direction: DriftDirection,
    /// Seconds the drift has been held.
    pub charge_s: f32,
}

impl Drift {
    /// The mini-turbo tier the charge has reached: the boost a release now
    /// would give. Sparks at the wheels show it.
    pub fn mini_turbo(&self) -> MiniTurbo {
        use tuning::{MINI_TURBO_ORANGE_CHARGE_S, MINI_TURBO_RED_CHARGE_S};
        if self.charge_s >= MINI_TURBO_RED_CHARGE_S {
            MiniTurbo::Red
        } else if self.charge_s >= MINI_TURBO_ORANGE_CHARGE_S {
            MiniTurbo::Orange
        } else {
            MiniTurbo::None
        }
    }
}

/// The mini-turbo tiers a drift charges through, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MiniTurbo {
    None,
    Orange,
    Red,
}

impl MiniTurbo {
    /// The tier `drift` has charged; `MiniTurbo::None` out of a drift.
    pub fn of(drift: Option<Drift>) -> Self {
        drift.map_or(Self::None, |drift| drift.mini_turbo())
    }

    /// The tier as one byte in charge order, for the hash and the bridge.
    pub fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Orange => 1,
            Self::Red => 2,
        }
    }

    /// How long releasing this tier boosts the kart.
    fn boost_s(self) -> f32 {
        use tuning::{MINI_TURBO_ORANGE_BOOST_S, MINI_TURBO_RED_BOOST_S};
        match self {
            Self::None => 0.0,
            Self::Orange => MINI_TURBO_ORANGE_BOOST_S,
            Self::Red => MINI_TURBO_RED_BOOST_S,
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
    /// The kart picked up `coin`, an index into the track's coins, which is
    /// gone for the tuned time, and carries one more.
    CoinPicked { kart: usize, coin: usize },
    /// The kart hit a hazard `by` fired, or was hit by a projectile `by`
    /// fired, or, with `by` `None`, was reached by the runaway wheel, and
    /// is spun out: stopped, with its inputs ignored, for the tuned time.
    SpunOut { kart: usize, by: Option<usize> },
    /// The spin-out just reported cost the kart `count` coins, the tuned
    /// number or all it carried; they vanish. Not reported for a kart that
    /// carried none.
    CoinsLost { kart: usize, count: u32 },
    /// The race ended: the standings are the results and the outro begins.
    RaceEnded,
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
    /// finish times are read against. The race ends once no kart holds it
    /// open: every human's kart has finished or is driven by a bot (ADR
    /// 0032). From the first human's finish `cap_left_s` counts down the
    /// tuned cap, and the race ends when it runs out whoever is racing.
    Racing {
        elapsed_s: f32,
        cap_left_s: Option<f32>,
    },
    /// The race has ended and the standings are the results, final. For
    /// the tuned outro a bot drives every kart, with no items and no laps
    /// counted; `remaining_s` counts it down.
    Outro { remaining_s: f32 },
    /// Over: the standings are the results and `step` changes nothing.
    Finished,
}

impl Phase {
    /// The phase as one byte in lifecycle order, for the hash and the bridge.
    pub fn code(self) -> u8 {
        match self {
            Self::Countdown { .. } => 0,
            Self::Racing { .. } => 1,
            Self::Outro { .. } => 2,
            Self::Finished => 3,
        }
    }
}

/// What a kart's rocket start comes to (GLOSSARY): during the countdown the
/// tier the throttle hold so far has reached, so the tiers come as the hold
/// lengthens and a hold that has tipped into a stall reads as one; from Go
/// the verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
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
    /// edge is read to the nearest half tick: a hold that began as the
    /// fourth light came on is the full boost, not the stall.
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
/// what race position is ordered by. One per kart from [`State::standings`],
/// serialised in the shape of the generated TypeScript type (ADR 0026).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Standing {
    /// Race position, 1 for the leader. Finishers first in finishing order,
    /// then the rest by laps completed, then checkpoint, then spline
    /// progress; a dead heat keeps grid order.
    pub position: u32,
    /// The race clock as the kart completed the race's laps; `None` while
    /// it is still racing. Final once set: nothing after the line moves a
    /// finisher's place.
    pub finish_time_s: Option<f32>,
    #[serde(skip)]
    pub laps_completed: u32,
    /// The lap the kart is on, from 1; a kart that has completed the race's
    /// laps stays on the last one.
    pub lap: u32,
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

/// Everything the sim knows about a race. Owns the physics world and the
/// RNG, so a `State` plus an input sequence replays identically anywhere.
pub struct State {
    /// What the race was started from: the item generator's seed, and the
    /// runaway wheel's landings.
    seed: u64,
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
    /// Seconds until each of the track's coins is back, in the track's
    /// order; zero or less while the coin is there to pick up.
    coin_respawn_s: Vec<f32>,
    /// The hazards on the track, in the order they were fired.
    hazards: Vec<PlacedHazard>,
    /// The projectiles in flight, in the order they were fired.
    projectiles: Vec<FlyingProjectile>,
    /// Each kart's race position, in grid order, as the race ended; `None`
    /// until it has. The outro moves the karts but not the results.
    final_positions: Option<Vec<u32>>,
}

#[derive(Clone)]
struct Kart {
    body: RigidBodyHandle,
    collider: ColliderHandle,
    /// The bot driving this kart, with what it remembers; `None` while its
    /// human drives it.
    bot: Option<bot::Memory>,
    /// How the kart answers its controls: its speed and everything that
    /// bends it.
    handling: Handling,
    /// Seconds the kart has been off the course without a break, outside
    /// the world or behind a wall; it respawns when this reaches the
    /// tuned delay.
    off_course_s: f32,
    /// Seconds spline progress has been falling without rising; the
    /// wrong-way warning shows once this reaches the tuned time. Standing
    /// still neither adds to it nor clears it.
    wrong_way_s: f32,
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
    /// finished kart counts no more laps, its lap clock stops, and a bot
    /// drives it, taking and firing no items.
    finish_time_s: Option<f32>,
    /// The item in the kart's one slot. While it holds one it drives over
    /// item boxes without taking them.
    held_item: Option<Item>,
    /// The coins the kart carries, never more than a full purse.
    coins: u32,
    /// Whether the item button was down last tick, so a held button fires
    /// once, as it goes down, and holding it through a pickup fires nothing.
    item_was_held: bool,
    /// Seconds of spin-out left after a hit; the kart is stopped and dead to
    /// every control until it runs out. Zero or less when not spun out.
    spin_out_left_s: f32,
    /// The last runaway wheel pass that hit the kart, from 1; 0 before any
    /// has. A pass hits a kart once, however long it stands in its path.
    wheel_pass_hit: u32,
}

impl Kart {
    fn finished(&self) -> bool {
        self.finish_time_s.is_some()
    }

    fn spun_out(&self) -> bool {
        self.spin_out_left_s > 0.0
    }

    /// Dead to every control: stalled at Go or spun out by an item.
    fn ignores_inputs(&self) -> bool {
        self.stall_left_s > 0.0 || self.spun_out()
    }

    /// What the coins the kart carries multiply its forward top speed by.
    fn coin_factor(&self) -> f32 {
        coin_factor(self.coins)
    }

    /// The lap the kart is on, from 1; a kart that has completed the race's
    /// laps stays on the last one.
    fn lap(&self) -> u32 {
        (self.laps_completed + 1).min(LAPS_PER_RACE)
    }

    /// What the kart reports, with `body` its body and `standing` its
    /// standing as of the same tick.
    fn readout(&self, body: &RigidBody, standing: &Standing) -> KartReadout {
        KartReadout {
            position: body.translation().to_array(),
            rotation: body.rotation().to_array(),
            speed: self.handling.speed(),
            velocity: body.linvel().to_array(),
            bump: {
                let bump = self.handling.bump();
                (bump != Vec3::ZERO).then_some(bump.to_array())
            },
            drift: self.handling.drift(),
            mini_turbo: MiniTurbo::of(self.handling.drift()),
            held_item: self.held_item,
            coins: self.coins,
            spin_out_s: self.spun_out().then_some(self.spin_out_left_s),
            airborne: self.handling.airborne(),
            boost_s: self.handling.boost_s(),
            boost_tier: self.handling.boost_tier(),
            start: self.start,
            stall_s: (self.stall_left_s > 0.0).then_some(self.stall_left_s),
            race_position: standing.position,
            lap: standing.lap,
            checkpoint: standing.checkpoint,
            spline_progress: standing.spline_progress,
            wrong_way: standing.wrong_way,
        }
    }

    /// Take on the word of `readout` on everything a snapshot carries, at
    /// `clock`, the race clock at the snapshot's tick: the inverse of
    /// [`Kart::readout`] for the fields the kart keeps beside the body.
    /// What a snapshot does not carry, and the boost, which it carries for
    /// the flame, stay the kart's own, run by the same rules on the same
    /// inputs. From Go, a rocket start verdict other than the kart's
    /// replaces the boost or stall it was given at Go with the
    /// readout's, run down by the ticks since. The caller puts the body
    /// where the readout has it.
    fn adopt(&mut self, readout: &KartReadout, clock: Phase) {
        self.handling.adopt(
            Rotation::from_array(readout.rotation).normalize(),
            readout.drift,
            Vec3::from(readout.velocity),
            readout.bump.map_or(Vec3::ZERO, Vec3::from),
        );
        self.held_item = readout.held_item;
        self.coins = readout.coins;
        // A spin-out the readout does not have is a hit the server never
        // made, which in a prediction is only ever the wheel's: the pass
        // that made it may still hit the kart where the server has it.
        if self.spun_out() && readout.spin_out_s.is_none() {
            self.wheel_pass_hit = 0;
        }
        self.spin_out_left_s = readout.spin_out_s.unwrap_or(0.0);
        if let Phase::Racing { elapsed_s, .. } = clock {
            if self.start != readout.start {
                self.start = readout.start;
                self.handling.end_boost();
                apply_start(self, readout.start);
                let ticks_since_go = (elapsed_s / tuning::TICK_S).round() as u32;
                self.handling.spend_boost(ticks_since_go, tuning::TICK_S);
            }
        }
        self.stall_left_s = readout.stall_s.unwrap_or(0.0);
        if self.lap() != readout.lap {
            self.laps_completed = readout.lap.saturating_sub(1);
        }
        self.checkpoint = readout.checkpoint;
        self.spline_progress = readout.spline_progress;
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

use tuning::{
    coin_factor, COINS_LOST_PER_SPIN_OUT, COIN_CAP, COIN_HALF_LENGTH_M, COIN_HALF_WIDTH_M,
    COIN_RESPAWN_S,
};
use tuning::{BOOST_PAD_FACTOR, RESPAWN_DELAY_S, WRONG_WAY_WARNING_S};
use tuning::{BOT_RUBBER_BAND_MAX_FACTOR, BOT_RUBBER_BAND_MIN_FACTOR, BOT_RUBBER_BAND_RANGE_M};
use tuning::{
    BOT_START_BOOST_CHANCE, BOT_START_HOLD_S, COUNTDOWN_S, RACE_END_CAP_S, RACE_OUTRO_S,
    START_BOOST_FACTOR, START_BOOST_S, START_FULL_BOOST_HOLD_S, START_STALL_HOLD_S, START_STALL_S,
    START_THROTTLE_HELD, START_WEAK_BOOST_FACTOR, START_WEAK_BOOST_HOLD_S, START_WEAK_BOOST_S,
};
use tuning::{
    GRAVITY_M_PER_S2, GROUND_CONTACT_M, KART_EDGE_RADIUS_M, KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M,
    KART_HALF_WIDTH_M, LAPS_PER_RACE, TOP_SPEED_M_PER_S,
};
use tuning::{
    HAZARD_DROP_DISTANCE_M, HAZARD_LIFETIME_S, HAZARD_RADIUS_M, ITEM_BOOST_FACTOR, ITEM_BOOST_S,
    ITEM_BOX_HALF_LENGTH_M, ITEM_BOX_HALF_WIDTH_M, ITEM_BOX_RESPAWN_S, ITEM_ROLL_PERCENT,
    ITEM_ROLL_WEIGHTS, SPIN_OUT_S, WHEEL_RADIUS_M,
};
use tuning::{
    HOMING_LOCK_RANGE_M, HOMING_LOOKAHEAD_M, HOMING_RADIUS_M, HOMING_SPEED_FACTOR,
    HOMING_WITHOUT_TARGET_IS_WASTED,
};
use tuning::{
    KART_BUMP_RESTITUTION, KART_BUMP_SPEED_LOSS, TYRE_WALL_RESTITUTION, TYRE_WALL_SPEED_LOSS,
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

/// Stands in for a missing best lap, finish time or cap in the hash; none
/// of them is ever negative.
const NO_TIME: f32 = -1.0;

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
                    handling: Handling::new(),
                    off_course_s: 0.0,
                    wrong_way_s: 0.0,
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
                    coins: 0,
                    item_was_held: false,
                    spin_out_left_s: 0.0,
                    wheel_pass_hit: 0,
                }
            })
            .collect();
        physics.index_colliders();

        Self {
            seed,
            rng,
            item_box_respawn_s: vec![0.0; track.item_boxes().len()],
            coin_respawn_s: vec![0.0; track.coins().len()],
            hazards: Vec::new(),
            projectiles: Vec::new(),
            final_positions: None,
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

    /// The runaway wheel at each of the track's crossings, in the track's
    /// order, as of the last tick: quiet unless the race is being raced.
    pub fn wheels(&self) -> Vec<Wheel> {
        (0..self.track.wheel_crossings().len())
            .map(|crossing| match self.phase {
                Phase::Racing { elapsed_s, .. } => {
                    wheel(&self.track, crossing, self.seed, elapsed_s)
                }
                _ => Wheel::Quiet,
            })
            .collect()
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

    /// Whether the bot in `slot` held back for a runaway wheel on the last
    /// tick; never for a human's kart.
    pub fn bot_held_for_the_wheel(&self, slot: usize) -> bool {
        self.karts[slot]
            .bot
            .is_some_and(|memory| memory.held_for_the_wheel())
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

    /// Whether each of the track's coins, in the track's order, is there to
    /// pick up as of the last tick; a picked-up coin is back the tuned time
    /// after it was taken.
    pub fn coins_present(&self) -> Vec<bool> {
        self.coin_respawn_s
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
    pub fn projectiles(&self) -> Vec<Projectile> {
        self.projectiles
            .iter()
            .map(|flying| flying.projectile)
            .collect()
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

    /// The human drives their kart in `slot` again from the next tick,
    /// unless it has finished or the race has ended: a bot keeps those.
    /// `slot` must be a human's.
    pub fn hand_back(&mut self, slot: usize) {
        assert!(slot < self.humans, "kart {slot} is not a human's");
        let over = matches!(self.phase, Phase::Outro { .. } | Phase::Finished);
        let kart = &mut self.karts[slot];
        if !over && !kart.finished() {
            kart.bot = None;
        }
    }

    /// Whether the end of the race waits on the kart in `slot`: a human's
    /// kart its human drives, or in a race with no humans any kart. Its
    /// finish starts the cap, and while it races the race stays open.
    fn contends(&self, slot: usize) -> bool {
        self.humans == 0 || (slot < self.humans && self.karts[slot].bot.is_none())
    }

    /// Whether a contending kart is still racing.
    fn held_open(&self) -> bool {
        (0..self.karts.len()).any(|slot| self.contends(slot) && !self.karts[slot].finished())
    }

    /// End the race: the standings as they are become the results, and a
    /// bot drives every kart through the outro.
    fn end_race(&mut self) {
        let positions = self
            .standings()
            .iter()
            .map(|standing| standing.position)
            .collect();
        self.final_positions = Some(positions);
        for kart in &mut self.karts {
            kart.bot.get_or_insert_with(bot::Memory::default);
        }
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
        let kart = &self.karts[kart];
        self.surface_at(self.physics.bodies[kart.body].translation().to_array())
    }

    /// A hash of everything that influences future ticks: the phase and its
    /// clocks, the results once the race has ended, the RNG, the item boxes'
    /// and the coins' respawn clocks, the hazards and projectiles and, per
    /// kart, its handling, its item, its coins and its body's position,
    /// heading, velocities and sleep state, including the timer that puts a
    /// resting body to sleep.
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
                cap_left_s,
            } => {
                hasher.write_f32(elapsed_s);
                hasher.write_f32(cap_left_s.unwrap_or(NO_TIME));
            }
            Phase::Outro { remaining_s } => hasher.write_f32(remaining_s),
            Phase::Finished => {}
        }
        if let Some(positions) = &self.final_positions {
            for &position in positions {
                hasher.write_u32(position);
            }
        }
        hasher.write_u64(self.seed);
        hasher.write(&self.rng.get_seed());
        hasher.write_u64(self.rng.get_stream());
        hasher.write_u128(self.rng.get_word_pos());
        hasher.write_f32s(self.item_box_respawn_s.iter().copied());
        hasher.write_f32s(self.coin_respawn_s.iter().copied());
        hasher.write_u64(self.hazards.len() as u64);
        for placed in &self.hazards {
            hasher.write_f32s(placed.hazard.position);
            hasher.write_u64(placed.hazard.owner as u64);
            hasher.write_u8(u8::from(placed.hazard.held));
            hasher.write_f32(placed.lifetime_left_s);
            hasher.write_u8(u8::from(placed.clear_of_owner));
        }
        hasher.write_u64(self.projectiles.len() as u64);
        for flying in &self.projectiles {
            hasher.write_f32s(flying.projectile.position);
            hasher.write_u64(flying.projectile.owner as u64);
            hasher.write_u64(flying.projectile.target as u64);
            hasher.write_f32(flying.progress);
        }
        for kart in &self.karts {
            let body = &self.physics.bodies[kart.body];
            kart.handling.hash(&mut hasher);
            hasher.write_f32(kart.off_course_s);
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
            hasher.write_u32(kart.coins);
            hasher.write_u8(u8::from(kart.item_was_held));
            hasher.write_f32(kart.spin_out_left_s);
            hasher.write_u32(kart.wheel_pass_hit);
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
    /// last tick so an overtake shows the tick it happens; once the race
    /// has ended, as it ended.
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
        if let Some(final_positions) = &self.final_positions {
            positions.clone_from(final_positions);
        }
        self.karts
            .iter()
            .zip(positions)
            .map(|(kart, position)| Standing {
                position,
                finish_time_s: kart.finish_time_s,
                laps_completed: kart.laps_completed,
                lap: kart.lap(),
                checkpoint: kart.checkpoint,
                spline_progress: kart.spline_progress,
                lap_time_s: kart.lap_time_s,
                best_lap_s: kart.best_lap_s,
                wrong_way: kart.wrong_way_s >= WRONG_WAY_WARNING_S,
            })
            .collect()
    }

    /// One readout per kart, in grid order, as of the last tick.
    pub fn kart_readouts(&self) -> Vec<KartReadout> {
        self.karts
            .iter()
            .zip(self.standings())
            .map(|(kart, standing)| kart.readout(&self.physics.bodies[kart.body], &standing))
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
/// holds nothing, so it starts clean. Through the outro the karts drive on
/// under their bots and nothing else is raced; once it is over the state is
/// returned as it was.
pub fn step(mut state: State, inputs: &[KartInput], dt: f32) -> State {
    assert_eq!(inputs.len(), state.humans, "one input per human kart");
    state.events.clear();
    // The race clock of a racing tick; `None` through the outro.
    let elapsed_s = match state.phase {
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
        Phase::Racing { elapsed_s, .. } => Some(elapsed_s + dt),
        Phase::Outro { .. } => None,
    };
    // Skip building the readouts when no bot needs them.
    let inputs: Vec<KartInput> = if state.karts.iter().all(|kart| kart.bot.is_none()) {
        inputs.to_vec()
    } else {
        let readouts = state.kart_readouts();
        let standings = state.standings();
        let clock = state.phase;
        let seed = state.seed;
        let track = &state.track;
        state
            .karts
            .iter_mut()
            .enumerate()
            .map(|(slot, kart)| match &mut kart.bot {
                Some(memory) => {
                    bot::drive(track, &readouts, &standings, clock, seed, slot, memory, dt)
                }
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
        let out_of_items = kart.finished() || elapsed_s.is_none();
        if kart.ignores_inputs() || out_of_items || !pressed {
            continue;
        }
        if let Some(item) = kart.held_item.take() {
            match item {
                Item::Boost => kart
                    .handling
                    .boost(ITEM_BOOST_S, ITEM_BOOST_FACTOR, MiniTurbo::Red),
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
                        let (position, progress) =
                            at_centreline_height(&state.track, here, kart.spline_progress);
                        state.projectiles.push(FlyingProjectile {
                            projectile: Projectile {
                                position,
                                owner: slot,
                                target,
                            },
                            progress,
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
        let kart = &state.karts[slot];
        if kart.held_item.is_some() || kart.finished() || elapsed_s.is_none() {
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
    // Coins come back on their clocks as the boxes do. A kart takes every
    // coin its centre is on, airborne or not, until it carries a full purse;
    // a spun-out kart takes none, nor, as with boxes, a finished kart or
    // any kart through the outro.
    for respawn_s in &mut state.coin_respawn_s {
        if *respawn_s > 0.0 {
            *respawn_s -= dt;
        }
    }
    for slot in 0..state.karts.len() {
        let kart = &mut state.karts[slot];
        if kart.spun_out() || kart.finished() || elapsed_s.is_none() {
            continue;
        }
        let position = state.physics.bodies[kart.body].translation().to_array();
        for (coin, placed) in state.track.coins().iter().enumerate() {
            if kart.coins >= COIN_CAP {
                break;
            }
            let respawn_s = &mut state.coin_respawn_s[coin];
            if *respawn_s <= 0.0 && placed.covers(position, COIN_HALF_LENGTH_M, COIN_HALF_WIDTH_M) {
                kart.coins += 1;
                *respawn_s = COIN_RESPAWN_S;
                state
                    .events
                    .push(RaceEvent::CoinPicked { kart: slot, coin });
            }
        }
    }
    let situations: Vec<Situation> = (0..state.karts.len())
        .map(|slot| {
            let body = &state.physics.bodies[state.karts[slot].body];
            let position = body.translation().to_array();
            Situation {
                rotation: *body.rotation(),
                vertical_m_per_s: body.linvel().y,
                surface: state.physics.surface_at(position),
                on_the_ground: state.physics.kart_on_the_ground(position),
                boost_pad: state.track.on_a_boost_pad(position),
                kerb: state.track.on_a_kerb(position),
                coin_factor: state.karts[slot].coin_factor(),
                top_speed: TOP_SPEED_M_PER_S
                    * state.rubber_band(slot)
                    * state.karts[slot].coin_factor(),
            }
        })
        .collect();
    // Whether each kart holds the item button this tick, as the controls
    // reach the handling: a kart dead to its controls holds nothing.
    let mut holds = vec![false; state.karts.len()];
    for (((kart, input), situation), hold) in state
        .karts
        .iter_mut()
        .zip(&inputs)
        .zip(situations)
        .zip(&mut holds)
    {
        // A stalled or spun-out kart sits dead to every control until its
        // time runs out; a spun-out one stands where it is, whatever runs
        // into it, and the tick its spin-out runs out it moves again.
        let freeze = if kart.ignores_inputs() {
            kart.stall_left_s -= dt;
            kart.spin_out_left_s -= dt;
            if kart.spun_out() {
                Freeze::Stopped
            } else {
                Freeze::Controls
            }
        } else {
            Freeze::Free
        };
        *hold = freeze == Freeze::Free && input.hold_item;
        let body = &mut state.physics.bodies[kart.body];
        body.lock_translations(kart.spun_out(), true);
        let sent = kart.handling.tick(input, freeze, situation, dt);
        body.set_linvel(sent.velocity, true);
        body.set_angvel(Vec3::Y * sent.yaw_rate, true);
    }
    let before: Vec<[f32; 3]> = state
        .karts
        .iter()
        .map(|kart| state.physics.bodies[kart.body].translation().to_array())
        .collect();
    // The velocities the handling sent the karts off with, and the bumps
    // in them, before the solver's contacts bent them: a touch between
    // karts is judged by the velocities, and a wall's hold reads the speed
    // with the bump taken off.
    let sent_velocities: Vec<Vec3> = state
        .karts
        .iter()
        .map(|kart| state.physics.bodies[kart.body].linvel())
        .collect();
    let sent_bumps: Vec<Vec3> = state
        .karts
        .iter()
        .map(|kart| kart.handling.bump())
        .collect();
    state.physics.step(dt);
    let mut finished_this_tick = Vec::new();
    let mut starts_the_cap = false;
    for (index, mut before) in before.into_iter().enumerate() {
        // Judged from the contacts and the sent velocities alone, so what
        // an earlier kart's pass did to its own body never reaches this.
        bump_off_karts(
            &mut state.karts,
            index,
            &state.physics,
            &sent_velocities,
            dt,
        );
        let contends = state.contends(index);
        let kart = &mut state.karts[index];
        // The wall holds first, so a kart against a wall and a tyre wall at
        // once loses the speed to both.
        held_by_walls(
            kart,
            &mut state.physics,
            sent_velocities[index],
            sent_bumps[index],
            dt,
        );
        bump_off_tyre_walls(kart, &state.physics, sent_velocities[index], dt);
        let mut after = state.physics.bodies[kart.body].translation().to_array();
        let mut respawned = false;
        let on_the_line = state.track.point_at(kart.spline_progress);
        if state.physics.surface_at(after) == Surface::OutOfWorld
            || state.physics.wall_between(on_the_line, after)
        {
            kart.off_course_s += dt;
            if kart.off_course_s >= RESPAWN_DELAY_S {
                after = respawn(kart, &state.track, &mut state.physics);
                state.events.push(RaceEvent::Respawned { kart: index });
                // A teleport is not a move: it crosses no gate.
                before = after;
                respawned = true;
            }
        } else {
            kart.off_course_s = 0.0;
        }
        // A respawn can land a kart anywhere on the lap, so its last reading
        // says nothing about where to search.
        let progress = if respawned {
            state.track.progress_at(after)
        } else {
            state.track.progress_near(after, kart.spline_progress)
        };
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
        let Some(elapsed_s) = elapsed_s else {
            continue;
        };
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
                    starts_the_cap |= contends;
                    // Over the line, the kart is the bot's: nothing it does
                    // can change the result for the karts still racing.
                    kart.bot.get_or_insert_with(bot::Memory::default);
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
            spin_out(
                kart,
                body,
                slot,
                Some(placed.hazard.owner),
                &mut state.events,
            );
            return false;
        }
        true
    });
    state.hazards = hazards;
    // A rolling wheel spins out every kart it reaches, once a pass: a kart
    // already spun out, or hit by this pass, is passed by.
    if let Some(elapsed_s) = elapsed_s {
        for crossing in 0..state.track.wheel_crossings().len() {
            let Wheel::Rolling { pass, position, .. } =
                wheel(&state.track, crossing, state.seed, elapsed_s)
            else {
                continue;
            };
            for (slot, kart) in state.karts.iter_mut().enumerate() {
                let body = &mut state.physics.bodies[kart.body];
                let reached = reaches(
                    body.translation(),
                    body.rotation(),
                    position,
                    WHEEL_RADIUS_M,
                );
                if !reached || kart.spun_out() || kart.wheel_pass_hit == pass {
                    continue;
                }
                spin_out(kart, body, slot, None, &mut state.events);
                kart.wheel_pass_hit = pass;
            }
        }
    }
    // A projectile flies after its target and is used up on reaching the
    // kart: on a hazard the kart trails, which goes with it, else on the
    // kart itself, which spins out unless it already is.
    let mut projectiles = std::mem::take(&mut state.projectiles);
    projectiles.retain_mut(|flying| {
        let target = &mut state.karts[flying.projectile.target];
        let body = &mut state.physics.bodies[target.body];
        fly(flying, &state.track, body.translation(), dt);
        let projectile = flying.projectile;
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
                spin_out(
                    target,
                    body,
                    projectile.target,
                    Some(projectile.owner),
                    &mut state.events,
                );
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
    state.phase = next_phase(&state, starts_the_cap, dt);
    if matches!(state.phase, Phase::Outro { .. }) && state.final_positions.is_none() {
        state.end_race();
        state.events.push(RaceEvent::RaceEnded);
    }
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
        Start::WeakBoost => kart.handling.boost(
            START_WEAK_BOOST_S,
            START_WEAK_BOOST_FACTOR,
            MiniTurbo::Orange,
        ),
        Start::FullBoost => kart
            .handling
            .boost(START_BOOST_S, START_BOOST_FACTOR, MiniTurbo::Red),
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
                    cap_left_s: None,
                }
            }
        }
        Phase::Racing {
            elapsed_s,
            cap_left_s,
        } => Phase::Racing {
            elapsed_s: elapsed_s + dt,
            cap_left_s,
        },
        Phase::Outro { remaining_s } => {
            let remaining_s = remaining_s - dt;
            if remaining_s > 0.0 {
                Phase::Outro { remaining_s }
            } else {
                Phase::Finished
            }
        }
        Phase::Finished => Phase::Finished,
    }
}

/// The phase after a tick of `dt`, read from `state` once its karts have
/// moved and before its phase changes. The cap starts on the tick
/// `starts_the_cap` is first true, and the race ends when it runs out or no
/// kart holds it open; the outro then runs down to the finish.
fn next_phase(state: &State, starts_the_cap: bool, dt: f32) -> Phase {
    let (elapsed_s, cap_left_s) = match state.phase {
        Phase::Racing {
            elapsed_s,
            cap_left_s,
        } => (elapsed_s + dt, cap_left_s),
        phase => return clock_after(phase, dt),
    };
    let outro = Phase::Outro {
        remaining_s: RACE_OUTRO_S,
    };
    if !state.held_open() {
        return outro;
    }
    let cap_left_s = match cap_left_s {
        Some(left) => Some(left - dt),
        None if starts_the_cap => Some(RACE_END_CAP_S),
        None => None,
    };
    if cap_left_s.is_some_and(|left| left <= 0.0) {
        return outro;
    }
    Phase::Racing {
        elapsed_s,
        cap_left_s,
    }
}

/// A contact point this close to a wall at the end of the tick counts as
/// touching it; contacts further off are the solver's look-ahead.
const WALL_TOUCH_M: f32 = 0.01;

/// The wall holds the kart: every wall it ends the tick against is handed
/// to its handling, with the velocity the solver left and the bump the
/// tick sent it off with, `sent_bump`, and the body takes the velocity the
/// handling leaves it.
fn held_by_walls(
    kart: &mut Kart,
    physics: &mut Physics,
    sent_velocity: Vec3,
    sent_bump: Vec3,
    dt: f32,
) {
    let velocity = physics.bodies[kart.body].linvel();
    let mut walls = Vec::new();
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
                walls.push(normal);
            }
        }
    }
    let stopped = kart.handling.hold_against_walls(walls, velocity, sent_bump);
    if stopped != velocity {
        physics.bodies[kart.body].set_linvel(stopped, true);
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

/// Karts bounce off each other: the kart in `index` that ended the tick
/// touching another is bumped away from it (see [`Handling::bumped`]) at
/// the speed the two closed at, judged by the velocities the handling sent
/// them off with. A spun-out kart takes no bump: it stands, and the other
/// kart bounces off it.
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
    let heading = karts[index].handling.heading();
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
        for (normal, closing_speed) in
            touches_this_tick(pair, collider, other_sent - sent_velocities[index], dt)
        {
            // Each kart of the pair takes half the bounce.
            karts[index].handling.bumped(
                normal,
                closing_speed,
                heading,
                KART_BUMP_RESTITUTION / 2.0,
                KART_BUMP_SPEED_LOSS,
            );
        }
    }
}

/// A kart that ended the tick touching a tyre wall bounces off it as off a
/// kart standing still (see [`Handling::bumped`]), at the speed it closed
/// with the tyre wall, judged by the velocity the handling sent it off with. A
/// spun-out kart stands where it is and takes no bump.
fn bump_off_tyre_walls(kart: &mut Kart, physics: &Physics, sent_velocity: Vec3, dt: f32) {
    if kart.spun_out() {
        return;
    }
    let heading = kart.handling.heading();
    for pair in physics.narrow_phase.contact_pairs_with(kart.collider) {
        let other = other_collider(pair, kart.collider);
        if physics.colliders[other].user_data != u128::from(Surface::Tyres.code()) {
            continue;
        }
        // The tyre wall stands still, so the kart closes at its own speed.
        for (normal, closing_speed) in touches_this_tick(pair, kart.collider, -sent_velocity, dt) {
            kart.handling.bumped(
                normal,
                closing_speed,
                heading,
                TYRE_WALL_RESTITUTION,
                TYRE_WALL_SPEED_LOSS,
            );
        }
    }
}

/// The touches `collider` made through `pair` this tick: for each contact
/// manifold, its normal along the ground pointing at `collider` and the
/// speed the two closed at along it, given `closing_velocity`, the other's
/// sent velocity less the collider's. Contact distances are from the start
/// of the tick; the sent velocities say whether the two met this tick,
/// before the solver held them apart.
fn touches_this_tick(
    pair: &ContactPair,
    collider: ColliderHandle,
    closing_velocity: Vec3,
    dt: f32,
) -> impl Iterator<Item = (Vec3, f32)> + '_ {
    pair.manifolds().iter().filter_map(move |manifold| {
        let normal = normal_toward(pair, collider, manifold);
        let normal = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
        if normal == Vec3::ZERO {
            return None;
        }
        let closing_speed = closing_velocity.dot(normal).max(0.0);
        manifold
            .points
            .iter()
            .any(|contact| contact.dist - closing_speed * dt <= WALL_TOUCH_M)
            .then_some((normal, closing_speed))
    })
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
    kart.handling.respawn(facing * Vec3::NEG_Z);
    kart.off_course_s = 0.0;
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

/// `point` brought to the height of the centreline nearest it within reach
/// of the `last` reading, where a projectile rides, and that spline progress.
fn at_centreline_height(track: &Track, point: Vec3, last: f32) -> ([f32; 3], f32) {
    let progress = track.progress_near(point.to_array(), last);
    let height = track.point_at(progress)[1];
    ([point.x, height, point.z], progress)
}

/// Fly the projectile one tick of `dt` seconds at the tuned speed: along
/// the centreline, steering for the point the tuned lookahead ahead of
/// where it is, until its target at `target` is within the tuned lock
/// range, then straight at the target. Seen from above; the height is the
/// centreline's where it ends up.
fn fly(flying: &mut FlyingProjectile, track: &Track, target: Vec3, dt: f32) {
    let here = Vec3::from(flying.projectile.position);
    let to_target = Vec3::new(target.x - here.x, 0.0, target.z - here.z);
    let aim = if to_target.length() <= HOMING_LOCK_RANGE_M {
        to_target
    } else {
        let ahead = track.point_at(flying.progress + HOMING_LOOKAHEAD_M);
        Vec3::new(ahead[0] - here.x, 0.0, ahead[2] - here.z)
    };
    let reach = HOMING_SPEED_FACTOR * TOP_SPEED_M_PER_S * dt;
    let moved = if aim.length() <= reach {
        aim
    } else {
        aim.normalize() * reach
    };
    (flying.projectile.position, flying.progress) =
        at_centreline_height(track, here + moved, flying.progress);
}

/// Stop the kart in `slot` where it is and start its spin-out, `by` the
/// kart whose item hit it or the runaway wheel: a drift or a boost ends, a
/// hop lands without a drift, and no control reaches it and no kart pushes
/// it until the spin-out runs out. Every spin-out costs the tuned coins, or
/// all the kart carries; both are reported to `events`.
fn spin_out(
    kart: &mut Kart,
    body: &mut RigidBody,
    slot: usize,
    by: Option<usize>,
    events: &mut Vec<RaceEvent>,
) {
    events.push(RaceEvent::SpunOut { kart: slot, by });
    let lost = kart.coins.min(COINS_LOST_PER_SPIN_OUT);
    if lost > 0 {
        kart.coins -= lost;
        events.push(RaceEvent::CoinsLost {
            kart: slot,
            count: lost,
        });
    }
    kart.spin_out_left_s = SPIN_OUT_S;
    kart.handling.stop();
    body.set_linvel(Vec3::ZERO, true);
    body.set_angvel(Vec3::ZERO, true);
    body.lock_translations(true, true);
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
        self.surface_below(point)
            .map_or(Surface::OutOfWorld, |(surface, _)| surface)
    }

    /// Whether a kart whose centre is at `point` stands on the track: the
    /// topmost surface under its centre is no further below its resting
    /// height than the tuned contact allowance.
    fn kart_on_the_ground(&self, point: [f32; 3]) -> bool {
        self.surface_below(point)
            .is_some_and(|(_, depth)| depth <= KART_HALF_HEIGHT_M + GROUND_CONTACT_M)
    }

    /// Whether a wall stands between `on_the_line`, a point on the
    /// centreline, and `kart`, seen level at a kart's height over the line:
    /// a kart a wall shuts out of the course, which no way back crosses.
    /// A kart held against a wall from the run-off is not behind it.
    fn wall_between(&self, on_the_line: [f32; 3], kart: [f32; 3]) -> bool {
        let from = Vec3::from(on_the_line) + Vec3::Y * KART_HALF_HEIGHT_M;
        let across = Vec3::new(kart[0] - from.x, 0.0, kart[2] - from.z);
        let distance = across.length();
        if distance == 0.0 {
            return false;
        }
        let walls_only = |_: ColliderHandle, collider: &Collider| {
            collider.user_data == u128::from(Surface::Wall.code())
        };
        let pipeline = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::only_fixed().predicate(&walls_only),
        );
        pipeline
            .cast_ray(&Ray::new(from, across / distance), distance, true)
            .is_some()
    }

    /// The topmost tagged surface at or below `point` and how far below it
    /// lies, or `None` when there is none. Karts are never an answer.
    fn surface_below(&self, point: [f32; 3]) -> Option<(Surface, f32)> {
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
        let (collider, toi) = pipeline.cast_ray(&ray, f32::MAX, true)?;
        let surface = Surface::from_code(self.colliders[collider].user_data as u8)?;
        Some((surface, toi - SURFACE_PROBE_RISE_M))
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
