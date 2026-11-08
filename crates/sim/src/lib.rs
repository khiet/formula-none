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
mod hits;
pub mod interpolation;
mod picks;
pub mod prediction;
pub mod replay;
mod stop;
pub mod track;
pub mod tuning;
mod weather;
mod wheel;

pub use bot::BotLevel;
pub use handling::Tyres;
pub use interpolation::Interpolation;
pub use picks::{PickChoices, Picks, UnofferedPick};
pub use prediction::Prediction;
pub use replay::{Replay, ReplayError, Segment};
pub use stop::{stop_roll, Stop, StopTier};
pub use track::{
    Checkpoint, PitLane, Placement, Surface, SurfaceMesh, Track, TrackError, WheelCrossing,
};
pub use weather::{rain_lap, rain_laps, Weather};
pub use wheel::{wheel, Wheel};

use handling::{Freeze, Handling, Situation};
pub(crate) use hits::HornBlast;
use hits::{Resolution, Source, Target};

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
/// The wire carries it as it is, so each fact is a field once and a
/// reading worked out from other fields is a method (ADR 0090). Its serde
/// shape is the generated TypeScript type; the transform also crosses the
/// wasm seam as a flat buffer for the View (ADR 0003).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase")]
pub struct KartReadout {
    /// Metres.
    pub position: [f32; 3],
    /// A unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Metres per second along the kart's travel, as its handling drives
    /// it; negative when backing up. Not worked out from `velocity`, which
    /// also holds the bump and a drift's slip.
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
    pub item_slot: ItemSlot,
    /// The coins the kart carries, from 0 to a full purse; each raises its
    /// forward top speed by the tuned factor.
    pub coins: u32,
    /// How much life the kart's tyres have left, as a percentage: 100 is
    /// new and 0 is gone. It holds at 100 in a race on Tyres Off (ADR 0080).
    pub tyre_life: f32,
    /// Seconds of spin-out left, while the kart is spun out: stopped, with
    /// its inputs ignored. The View spins it for the duration.
    pub spin_out_s: Option<f32>,
    /// Whether the kart is in a hop's airtime; the hop's tell. A snapshot
    /// carries it, so an interpolated kart is in the air as the earlier
    /// snapshot has it.
    pub airborne: bool,
    /// Whether the kart is in the air with a trick earned, which the landing
    /// pays in a boost (ADR 0102): the driver's flourish and its sound. A
    /// snapshot carries it, and a prediction's kart adopts it.
    pub trick: bool,
    /// Whether the kart is on the pit limiter, between a pit lane's lines
    /// (ADR 0081); the gear slot's PIT.
    pub in_pit_lane: bool,
    /// The kart's stop in its pit box, while it is in one (ADR 0082): guided
    /// onto its mark and held there, dead to every control and touching
    /// nothing, until green. The crew's, the light's and the stop timer's
    /// tell; a snapshot carries it and a prediction's kart adopts it.
    pub stop: Option<Stop>,
    /// Stops the kart has made, each ended on green, which seeds its next
    /// roll (ADR 0082). A snapshot carries it so a prediction that joined
    /// after a stop rolls the next one as the server does.
    pub stops: u32,
    /// The boost holding the kart over top speed, while a mini-turbo,
    /// boost pad, item or start boost does; the release's and the flame's
    /// tell.
    pub boost: Option<Boost>,
    /// Seconds of a star's run left, while the kart has one: faster, immune
    /// and spinning out every kart it touches. The shine's tell, and what
    /// every client works the rainbow's phase out from.
    pub star_s: Option<f32>,
    /// The slipstream the kart is in: charging in another's wake, or towed
    /// to a raised top speed cap; `None` out of both. The wind lines' tell
    /// for the own kart, and what a prediction's kart adopts from a
    /// snapshot, since only the server judges the wake (ADR 0077).
    pub slipstream: Option<Slipstream>,
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
    /// The last checkpoint crossed in order; 0 is the start line. A `u32`,
    /// not the sim's `usize`, so it reaches JavaScript as a number.
    pub checkpoint: u32,
    /// Metres along the centreline from the start line.
    pub spline_progress: f32,
    /// The wrong-way warning is up.
    pub wrong_way: bool,
}

impl KartReadout {
    /// The tier the drift has charged, the sparks' tell; `MiniTurbo::None`
    /// out of a drift.
    pub fn mini_turbo(&self) -> MiniTurbo {
        MiniTurbo::of(self.drift)
    }

    /// Seconds of boost left, while boosting.
    pub fn boost_s(&self) -> Option<f32> {
        self.boost.map(|boost| boost.left_s)
    }

    /// The tier the boost burns in as a mini-turbo tier; `MiniTurbo::None`
    /// while not boosting.
    pub fn boost_tier(&self) -> MiniTurbo {
        self.boost
            .map_or(MiniTurbo::None, |boost| boost.tier.into())
    }

    /// The readings the bridge hands JavaScript beside the readout: those
    /// worked out from it, and `surface`, the surface under the kart.
    pub fn readings(&self, surface: Surface) -> KartReadings {
        KartReadings {
            surface,
            mini_turbo: self.mini_turbo(),
            boost_s: self.boost_s(),
            boost_tier: self.boost_tier(),
        }
    }
}

/// A boost under way: the seconds it has left and the tier it burns in.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase")]
pub struct Boost {
    pub left_s: f32,
    pub tier: BoostTier,
}

/// The tier a boost burns in, the flame's: orange for an orange release or
/// a weak rocket start, red for every other boost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase")]
pub enum BoostTier {
    Orange,
    Red,
}

impl BoostTier {
    /// The tier a boost given in mini-turbo tier `tier` burns in: only an
    /// orange one burns orange.
    fn of(tier: MiniTurbo) -> Self {
        match tier {
            MiniTurbo::Orange => Self::Orange,
            MiniTurbo::Red | MiniTurbo::None => Self::Red,
        }
    }
}

impl From<BoostTier> for MiniTurbo {
    fn from(tier: BoostTier) -> Self {
        match tier {
            BoostTier::Orange => Self::Orange,
            BoostTier::Red => Self::Red,
        }
    }
}

/// What the bridge hands JavaScript beside each [`KartReadout`]: the
/// readings worked out from it, which the wire does not carry, and the
/// surface under the kart. The web app merges the two into one value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase")]
pub struct KartReadings {
    pub surface: Surface,
    /// See [`KartReadout::mini_turbo`].
    pub mini_turbo: MiniTurbo,
    /// See [`KartReadout::boost_s`].
    pub boost_s: Option<f32>,
    /// See [`KartReadout::boost_tier`].
    pub boost_tier: MiniTurbo,
}

/// The items a kart can hold: what an item box rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase")]
pub enum Item {
    /// A burst of speed: the tuned factor on top speed for the tuned time.
    Boost,
    /// Dropped behind the kart; spins out whoever hits it.
    Hazard,
    /// Follows the track to the kart ahead and spins it out.
    Homing,
    /// A run of the tuned time: a raised top speed, immunity from every
    /// item and the wheel, and a spin-out for every kart the kart touches.
    Star,
    /// One blast the tuned reach round the kart: spins out the karts in
    /// reach and uses up the hazards and projectiles in reach.
    Horn,
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

/// A hazard as the rules keep it: what the View sees, plus its spline
/// progress, how long it has left and whether its owner has got clear of
/// it since the drop.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PlacedHazard {
    hazard: Hazard,
    /// Its owner's spline progress at the drop, kept up while it trails:
    /// what a horn's blast judges its reach by.
    progress: f32,
    /// Seconds until a dropped hazard is gone; a held one does not age.
    lifetime_left_s: f32,
    /// Whether the owner has been out of the hazard's reach since firing
    /// it. Until then the owner cannot hit it: the drop puts it right
    /// behind the kart, and a hold keeps it there.
    clear_of_owner: bool,
}

/// What a kart's one item slot holds. A box taken into an empty slot
/// starts a roll that lands the item `ITEM_ROLL_S` later; the roll counts
/// as full, so nothing fires and no box rolls until it lands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ItemSlot {
    #[default]
    Empty,
    /// Rolling into `item`, which lands in `roll_left_s` seconds.
    Rolling { item: Item, roll_left_s: f32 },
    /// Holding `item`, ready to fire.
    Held(Item),
}

/// Where a kart stands with the slipstream (GLOSSARY): in another kart's
/// wake short of the charge, or towed. A tagged enum across the seams, as
/// `ItemSlot` is (ADR 0026).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Slipstream {
    /// In a wake for `s` seconds so far, short of the charge.
    Charging { s: f32 },
    /// Towed, with `left_s` seconds of tow left: put back to the full
    /// tuned time every tick the kart is in a wake with a full charge,
    /// running down every other tick.
    Tow { left_s: f32 },
}

impl ItemSlot {
    /// The item ready to fire; `None` while the slot is empty or rolling.
    pub fn held(self) -> Option<Item> {
        match self {
            Self::Held(item) => Some(item),
            _ => None,
        }
    }

    /// The item held or rolling into the slot; `None` while it is empty.
    pub fn item(self) -> Option<Item> {
        match self {
            Self::Empty => None,
            Self::Rolling { item, .. } | Self::Held(item) => Some(item),
        }
    }

    /// The slot a tick of `dt` seconds later: a roll runs down, and lands
    /// once it has run out.
    fn after(self, dt: f32) -> Self {
        match self {
            Self::Rolling { item, roll_left_s } if roll_left_s - dt <= 0.0 => Self::Held(item),
            Self::Rolling { item, roll_left_s } => Self::Rolling {
                item,
                roll_left_s: roll_left_s - dt,
            },
            slot => slot,
        }
    }
}

impl Item {
    /// The item as one byte, for the hash and the bridge; 0 is left for an
    /// empty slot.
    pub fn code(self) -> u8 {
        match self {
            Self::Boost => 1,
            Self::Hazard => 2,
            Self::Homing => 3,
            Self::Star => 4,
            Self::Horn => 5,
        }
    }

    /// The item a roll in `0..ITEM_ROLL_PERCENT` picks for a kart in race `position`,
    /// from the tuned weights of the position's band: the first `hazard`
    /// percent are the hazard, the next `boost` the boost, the next `homing`
    /// the homing projectile, the next `star` the star and the rest the
    /// horn. A position past the last band rolls as the last.
    fn rolled(position: u32, roll: u32) -> Self {
        let band = ITEM_ROLL_WEIGHTS
            .iter()
            .find(|band| position <= u32::from(band.positions.1))
            .or(ITEM_ROLL_WEIGHTS.last())
            .expect("the roll table has a band");
        let hazard = u32::from(band.hazard);
        let boost = hazard + u32::from(band.boost);
        let homing = boost + u32::from(band.homing);
        let star = homing + u32::from(band.star);
        if roll < hazard {
            Self::Hazard
        } else if roll < boost {
            Self::Boost
        } else if roll < homing {
            Self::Homing
        } else if roll < star {
            Self::Star
        } else {
            Self::Horn
        }
    }
}

/// Which way a drift turns the kart. Fixed by the steer at the hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
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
#[ts(export_to = "../sim/")]
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
    /// The kart picked up `item_box`, an index into the track's item boxes,
    /// which is gone for the tuned time. With its slot empty it rolled `item`
    /// into it; holding an item it rolled nothing, and `item` is `None`.
    ItemPicked {
        kart: usize,
        item: Option<Item>,
        item_box: usize,
    },
    /// The kart fired the item it held; its slot is empty again.
    ItemFired { kart: usize, item: Item },
    /// The kart picked up `coin`, an index into the track's coins, which is
    /// gone for the tuned time, and carries one more.
    CoinPicked { kart: usize, coin: usize },
    /// The kart hit a hazard `by` fired, or was hit by a projectile `by`
    /// fired or touched by `by` on a star's run, or, with `by` `None`, was
    /// reached by the runaway wheel, and is spun out: stopped, with its
    /// inputs ignored, for the tuned time.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
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
#[ts(export_to = "../sim/")]
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
    /// Stops the kart has made in its pit box, each ended on green (ADR 0082).
    pub stops: u32,
    /// The shortest of those stops' stationary times; `None` before the first.
    pub best_stop_s: Option<f32>,
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
    /// The creator's picks. Fixed for the race, not part of what a tick
    /// changes, so not hashed.
    picks: Picks,
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
    /// Whether `step` judges each kart's wake from the bodies. A
    /// prediction's race holds the own kart alone, with the others drawn
    /// 100 ms behind, so it cannot: there the slipstream is the snapshots'
    /// word (ADR 0077). Fixed for the race, so not hashed.
    wakes_judged: bool,
    /// Ticks stepped so far, the server's tick numbers. Hashed only while it
    /// rains, the one time a later tick reads it.
    tick: u32,
    /// The lap that begins the rain when a kart starts it; `None` in a dry
    /// race, and in a prediction's, whose lone kart cannot say when the first
    /// kart got there (ADR 0103). Fixed for the race, so not hashed.
    rain_lap: Option<u32>,
    /// The tick the rain began; `None` until it has, so a dry race hashes as
    /// one from before there was weather.
    rain_began_tick: Option<u32>,
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
    /// The laps that finish this race. Out of the hash, like the Tyres pick,
    /// so a three-lap race hashes as one from before there was a pick (ADR 0083).
    race_laps: u32,
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
    /// Unless it is empty, a box the kart takes leaves it as it is.
    item_slot: ItemSlot,
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
    /// Seconds of a star's run left; zero or less off a run. Its own timer
    /// and not a boost, so a mini-turbo or a pad during the run does not
    /// end it and the brake still works; outside what a respawn clears, so
    /// a respawn does not stop it.
    star_left_s: f32,
    /// Where the kart last was on a pit lane's road, metres along it from
    /// the fork, kept while it is off the course, so a respawn puts it back
    /// there; `None` once it stands anywhere else on the course. Hashed only
    /// while there is one, so a race that never sees a lane hashes as before.
    pit_lane_m: Option<f32>,
    /// The kart's grid slot, which names its pit box and seeds its stops.
    /// Fixed for the race, so not hashed.
    slot: usize,
    /// The stop under way in the kart's pit box, if one is (ADR 0082); the
    /// kart is dead to every control and touches nothing until green.
    /// Hashed only while there is one, as `pit_lane_m` is.
    stop: Option<Stop>,
    /// Stops ended on green so far, which seeds the next one's roll, and the
    /// shortest stationary time among them. Hashed only once there is one.
    stops: u32,
    best_stop_s: Option<f32>,
}

impl Kart {
    fn finished(&self) -> bool {
        self.finish_time_s.is_some()
    }

    fn spun_out(&self) -> bool {
        self.spin_out_left_s > 0.0
    }

    /// On a star's run: faster, immune, and spinning out every kart it touches.
    fn starring(&self) -> bool {
        self.star_left_s > 0.0
    }

    /// In its pit box for a stop, from the catch to green.
    fn stopping(&self) -> bool {
        self.stop.is_some()
    }

    /// Held where it stands, whatever runs into it: spun out, or in its pit
    /// box for a stop.
    fn held_still(&self) -> bool {
        self.spun_out() || self.stopping()
    }

    /// Dead to every control: stalled at Go, spun out by an item or in its
    /// pit box for a stop.
    fn ignores_inputs(&self) -> bool {
        self.stall_left_s > 0.0 || self.held_still()
    }

    /// What the coins the kart carries multiply its forward top speed by.
    fn coin_factor(&self) -> f32 {
        coin_factor(self.coins)
    }

    /// The lap the kart is on, from 1; a kart that has completed the race's
    /// laps stays on the last one.
    fn lap(&self) -> u32 {
        (self.laps_completed + 1).min(self.race_laps)
    }

    /// What the kart reports, with `body` its body and `standing` its
    /// standing as of the same tick, on `track`.
    fn readout(&self, body: &RigidBody, standing: &Standing, track: &Track) -> KartReadout {
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
            item_slot: self.item_slot,
            coins: self.coins,
            tyre_life: self.handling.tyre_life(),
            spin_out_s: self.spun_out().then_some(self.spin_out_left_s),
            airborne: self.handling.airborne(),
            trick: self.handling.trick(),
            in_pit_lane: track.in_pit_lane(body.translation().to_array()),
            stop: self.stop,
            stops: self.stops,
            boost: self.handling.boost_s().map(|left_s| Boost {
                left_s,
                tier: BoostTier::of(self.handling.boost_tier()),
            }),
            slipstream: self.handling.slipstream(),
            star_s: self.starring().then_some(self.star_left_s),
            start: self.start,
            stall_s: (self.stall_left_s > 0.0).then_some(self.stall_left_s),
            race_position: standing.position,
            lap: standing.lap,
            checkpoint: standing.checkpoint as u32,
            spline_progress: standing.spline_progress,
            wrong_way: standing.wrong_way,
        }
    }

    /// Take on the word of `readout` on everything a snapshot carries, at
    /// `clock`, the race clock at the snapshot's tick: the inverse of
    /// [`Kart::readout`] for the fields the kart keeps beside the body.
    /// What a snapshot does not carry, the boost, which it carries for the
    /// flame, and the hop, which it carries for the other karts' tells, stay
    /// the kart's own, run by the same rules on the same inputs. From Go, a rocket start verdict other than the kart's
    /// replaces the boost or stall it was given at Go with the
    /// readout's, run down by the ticks since. The caller puts the body
    /// where the readout has it.
    fn adopt(&mut self, readout: &KartReadout, clock: Phase) {
        // First: the motion is read against what these tyres can slide.
        self.handling.adopt_tyre_life(readout.tyre_life);
        self.handling.adopt(
            Rotation::from_array(readout.rotation).normalize(),
            readout.drift,
            Vec3::from(readout.velocity),
            readout.bump.map_or(Vec3::ZERO, Vec3::from),
        );
        self.item_slot = readout.item_slot;
        self.coins = readout.coins;
        self.handling.adopt_slipstream(readout.slipstream);
        self.handling.adopt_trick(readout.trick);
        // A spin-out the readout does not have is a hit the server never
        // made, which in a prediction is only ever the wheel's: the pass
        // that made it may still hit the kart where the server has it.
        if self.spun_out() && readout.spin_out_s.is_none() {
            self.wheel_pass_hit = 0;
        }
        self.spin_out_left_s = readout.spin_out_s.unwrap_or(0.0);
        self.stop = readout.stop;
        self.stops = readout.stops;
        self.star_left_s = readout.star_s.unwrap_or(0.0);
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
        self.checkpoint = readout.checkpoint as usize;
        self.spline_progress = readout.spline_progress;
    }
}

/// Whether the kart on `slot` sits in another kart's wake, judged from
/// every kart's position and velocity in `motions`, in grid order, seen
/// from above (ADR 0077): some other kart travels at the leader's share of
/// the tuned top speed or more, this kart's centre lies from 0 to the
/// tuned range behind that kart's centre along its travel and within the
/// tuned half width of it across, and the two travel within the tuned
/// angle of each other. Travel is the body's velocity, not its facing, so
/// a drifting leader's wake lies along the road. Two wakes at once count
/// as one, and a kart that is not moving has no travel to compare, so it
/// is in no wake.
fn in_a_wake(motions: &[(Vec3, Vec3)], slot: usize) -> bool {
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let (position, velocity) = motions[slot];
    let Some(travel) = flat(velocity).try_normalize() else {
        return false;
    };
    // libm, not std, so native and wasm agree bit for bit (ADR 0002).
    let within_angle = libm::cosf(SLIPSTREAM_ANGLE_RAD);
    motions
        .iter()
        .enumerate()
        .filter(|&(other, _)| other != slot)
        .any(|(_, &(leader_position, leader_velocity))| {
            let leader_velocity = flat(leader_velocity);
            let leader_speed = leader_velocity.length();
            if leader_speed < SLIPSTREAM_LEADER_SPEED_FRACTION * TOP_SPEED_M_PER_S {
                return false;
            }
            let leader_travel = leader_velocity / leader_speed;
            let offset = flat(position - leader_position);
            let behind = -offset.dot(leader_travel);
            let across = (offset + leader_travel * behind).length();
            (0.0..=SLIPSTREAM_RANGE_M).contains(&behind)
                && across <= SLIPSTREAM_HALF_WIDTH_M
                && travel.dot(leader_travel) >= within_angle
        })
}

/// Marks a collider as a kart's, where a track collider carries its
/// surface's code; no surface code reaches this.
const KART_COLLIDER: u128 = u128::MAX;

/// The collision groups a kart's collider is in: an ordinary kart, or one
/// on a star's run. The track's colliders are in every group. Two karts
/// collide only when each is in the other's filter, so a star kart meets
/// the track and other star karts but passes through ordinary karts, which
/// in turn pass through it: no contact pair, so no bump (ADR 0021) for the
/// prediction and the server to disagree over. A kart in its pit box for a
/// stop is in no group and meets nothing: nobody can shunt it (ADR 0081).
const KART_GROUP: Group = Group::GROUP_1;
const STAR_GROUP: Group = Group::GROUP_2;

/// The groups a kart's collider is in while it is or is not `starring`,
/// unless it is `stopping`, when it is in none.
fn kart_groups(starring: bool, stopping: bool) -> InteractionGroups {
    if stopping {
        InteractionGroups::none()
    } else if starring {
        InteractionGroups::new(
            STAR_GROUP,
            Group::ALL.difference(KART_GROUP),
            InteractionTestMode::And,
        )
    } else {
        InteractionGroups::new(KART_GROUP, Group::ALL, InteractionTestMode::And)
    }
}

/// The box a kart collides with, as an ordinary kart. Its length runs
/// along -Z, its facing.
fn kart_collider() -> Collider {
    ColliderBuilder::round_cuboid(
        KART_HALF_WIDTH_M - KART_EDGE_RADIUS_M,
        KART_HALF_HEIGHT_M - KART_EDGE_RADIUS_M,
        KART_HALF_LENGTH_M - KART_EDGE_RADIUS_M,
        KART_EDGE_RADIUS_M,
    )
    .friction(0.0)
    .restitution(0.0)
    .collision_groups(kart_groups(false, false))
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

use tuning::BOT_RUBBER_BAND_RANGE_M;
use tuning::HOMING_WITHOUT_TARGET_IS_WASTED;
use tuning::{
    coin_factor, COINS_LOST_PER_SPIN_OUT, COIN_CAP, COIN_HALF_LENGTH_M, COIN_HALF_WIDTH_M,
    COIN_RESPAWN_S,
};
use tuning::{BOOST_PAD_FACTOR, RESPAWN_DELAY_S, WRONG_WAY_WARNING_S};
use tuning::{
    BOT_START_HOLD_S, COUNTDOWN_S, RACE_END_CAP_S, RACE_OUTRO_S, START_BOOST_FACTOR, START_BOOST_S,
    START_FULL_BOOST_HOLD_S, START_STALL_HOLD_S, START_STALL_S, START_THROTTLE_HELD,
    START_WEAK_BOOST_FACTOR, START_WEAK_BOOST_HOLD_S, START_WEAK_BOOST_S,
};
use tuning::{
    GRAVITY_M_PER_S2, GROUND_CONTACT_M, KART_EDGE_RADIUS_M, KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M,
    KART_HALF_WIDTH_M, TOP_SPEED_M_PER_S,
};
use tuning::{
    HAZARD_DROP_DISTANCE_M, HAZARD_LIFETIME_S, ITEM_BOOST_FACTOR, ITEM_BOOST_S,
    ITEM_BOX_HALF_LENGTH_M, ITEM_BOX_HALF_WIDTH_M, ITEM_BOX_RESPAWN_S, ITEM_ROLL_PERCENT,
    ITEM_ROLL_S, ITEM_ROLL_WEIGHTS, SPIN_OUT_S, STAR_KICK_FACTOR, STAR_KICK_S, STAR_S,
    STAR_TOP_SPEED_FACTOR,
};
use tuning::{
    KART_BUMP_RESTITUTION, KART_BUMP_SPEED_LOSS, TYRE_WALL_RESTITUTION, TYRE_WALL_SPEED_LOSS,
};
use tuning::{PIT_BOX_CATCH_M, PIT_BOX_GUIDE_S};
use tuning::{
    SLIPSTREAM_ANGLE_RAD, SLIPSTREAM_HALF_WIDTH_M, SLIPSTREAM_LEADER_SPEED_FRACTION,
    SLIPSTREAM_RANGE_M,
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
        Self::with_picks(seed, kart_count, 0, track, Picks::default())
    }

    /// As [`State::with_picks`], every pick but the bot level as before
    /// there were picks.
    pub fn with_bots(seed: u64, humans: usize, bots: usize, track: Track, level: BotLevel) -> Self {
        let picks = Picks {
            level,
            ..Picks::default()
        };
        Self::with_picks(seed, humans, bots, track, picks)
    }

    /// A fresh race on `track` with `humans` karts on the front of its spawn
    /// grid and `bots` karts on the slots behind them, run as `picks` says,
    /// at the top of its countdown. Together the karts must not exceed the
    /// track's spawn slots. `picks` must be offered ([`Picks::offered`]).
    pub fn with_picks(seed: u64, humans: usize, bots: usize, track: Track, picks: Picks) -> Self {
        let kart_count = humans + bots;
        assert!(
            kart_count <= track.spawn_grid().len(),
            "{} karts but the track has {} spawn slots",
            kart_count,
            track.spawn_grid().len()
        );
        Self::on_spawn_slots(seed, humans, 0..kart_count, track, picks)
    }

    /// A fresh race with one kart on each of `slots` of the spawn grid, in
    /// order, the first `humans` of them human, run as `picks` says, at the
    /// top of its countdown. Each bot kart's rocket start is rolled here, in
    /// slot order, so the seed decides it.
    fn on_spawn_slots(
        seed: u64,
        humans: usize,
        slots: Range<usize>,
        track: Track,
        picks: Picks,
    ) -> Self {
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

        let first_slot = slots.start;
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
                let bot_start_hold_s =
                    if index >= humans && rolls_the_start_boost(&mut rng, picks.level) {
                        BOT_START_HOLD_S
                    } else {
                        0.0
                    };
                Kart {
                    body,
                    collider,
                    bot: (index >= humans).then_some(bot::Memory::default()),
                    handling: Handling::new(picks.tyres),
                    off_course_s: 0.0,
                    wrong_way_s: 0.0,
                    laps_completed: 0,
                    race_laps: picks.laps,
                    checkpoint: 0,
                    spline_progress: track.progress_at(slot.position),
                    lap_time_s: 0.0,
                    best_lap_s: None,
                    start_hold_from_s: None,
                    start: Start::Clean,
                    bot_start_hold_s,
                    stall_left_s: 0.0,
                    finish_time_s: None,
                    item_slot: ItemSlot::Empty,
                    coins: 0,
                    item_was_held: false,
                    spin_out_left_s: 0.0,
                    wheel_pass_hit: 0,
                    star_left_s: 0.0,
                    pit_lane_m: None,
                    slot: first_slot + index,
                    stop: None,
                    stops: 0,
                    best_stop_s: None,
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
            picks,
            wakes_judged: true,
            tick: 0,
            rain_lap: (picks.weather == Weather::Rain).then(|| rain_lap(seed, picks.laps)),
            rain_began_tick: None,
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

    /// The tick the rain began, on the server's tick numbers; `None` until it
    /// has, and in a dry race always.
    pub fn rain_began_tick(&self) -> Option<u32> {
        self.rain_began_tick
    }

    /// How wet the track is, from 0 to 1, as of the last tick.
    pub fn wetness(&self) -> f32 {
        self.rain_began_tick
            .map_or(0.0, |began| weather::wetness(began, self.tick))
    }

    /// The creator's picks the race runs as, fixed for the race.
    pub fn picks(&self) -> Picks {
        self.picks
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

    /// Whether the bot in `slot` has called itself in to the pits; never for
    /// a human's kart.
    pub fn bot_pit_called(&self, slot: usize) -> bool {
        self.karts[slot]
            .bot
            .is_some_and(|memory| memory.pit_called())
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

    /// The kart in `slot` takes on `life` percent of tyre life from the next
    /// tick, as it would a snapshot's word: the seam the bots' lap costs are
    /// measured at (ADR 0087).
    pub fn adopt_tyre_life(&mut self, slot: usize, life: f32) {
        self.karts[slot].handling.adopt_tyre_life(life);
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
    /// bot drives every kart through the outro, with no items, so a star's
    /// run ends here as it does at the kart's own finish.
    fn end_race(&mut self) {
        let positions = self
            .standings()
            .iter()
            .map(|standing| standing.position)
            .collect();
        self.final_positions = Some(positions);
        for kart in &mut self.karts {
            kart.bot.get_or_insert_with(bot::Memory::default);
            kart.star_left_s = 0.0;
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
        let profile = self.picks.level.profile();
        if fraction > 0.0 {
            1.0 - fraction * (1.0 - profile.rubber_band_min_factor)
        } else {
            1.0 - fraction * (profile.rubber_band_max_factor - 1.0)
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
        if let Some(began) = self.rain_began_tick {
            hasher.write_u32(began);
            hasher.write_u32(self.tick);
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
            hasher.write_f32(placed.progress);
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
            hasher.write_u8(kart.item_slot.item().map_or(0, Item::code));
            if let ItemSlot::Rolling { roll_left_s, .. } = kart.item_slot {
                hasher.write_f32(roll_left_s);
            }
            hasher.write_u32(kart.coins);
            hasher.write_u8(u8::from(kart.item_was_held));
            hasher.write_f32(kart.spin_out_left_s);
            hasher.write_u32(kart.wheel_pass_hit);
            hasher.write_f32(kart.star_left_s);
            if let Some(metres) = kart.pit_lane_m {
                hasher.write_f32(metres);
            }
            if let Some(stop) = kart.stop {
                hasher.write_u8(stop.tier.code());
                hasher.write_f32(stop.stationary_s);
                hasher.write_f32(stop.guide_left_s);
                hasher.write_f32(stop.stationary_so_far_s);
            }
            if kart.stops > 0 {
                hasher.write_u32(kart.stops);
                hasher.write_f32(kart.best_stop_s.unwrap_or(NO_TIME));
            }
            if let Some(memory) = &kart.bot {
                hasher.write_f32s(memory.timers());
                // Only once a pit call or a stop has set them, so a race
                // that has neither hashes as it did before bots pitted.
                if let Some(pitting) = memory.pitting() {
                    hasher.write_f32s(pitting);
                }
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
                stops: kart.stops,
                best_stop_s: kart.best_stop_s,
            })
            .collect()
    }

    /// One readout per kart, in grid order, as of the last tick.
    pub fn kart_readouts(&self) -> Vec<KartReadout> {
        self.karts
            .iter()
            .zip(self.standings())
            .map(|(kart, standing)| {
                kart.readout(&self.physics.bodies[kart.body], &standing, &self.track)
            })
            .collect()
    }

    /// Whether the kart in `slot` stands where the pit limiter holds and the
    /// lane is safe ground (ADR 0081).
    fn in_pit_lane(&self, slot: usize) -> bool {
        let at = self.physics.bodies[self.karts[slot].body].translation();
        self.track.in_pit_lane(at.to_array())
    }

    /// The kart in `slot` as an item judges it, `in_pit_lane` or not.
    fn target(&self, slot: usize, in_pit_lane: bool) -> Target {
        let kart = &self.karts[slot];
        Target::of(kart, &self.physics.bodies[kart.body], in_pit_lane)
    }

    /// Spin out each kart `resolution` hits, in order, then despawn the
    /// items it used up.
    fn apply(&mut self, resolution: &Resolution) {
        for hit in &resolution.hits {
            let kart = &mut self.karts[hit.kart];
            let body = &mut self.physics.bodies[kart.body];
            spin_out(kart, body, hit.kart, hit.source.by(), &mut self.events);
            if let Source::Wheel(pass) = hit.source {
                kart.wheel_pass_hit = pass;
            }
        }
        resolution
            .despawns
            .apply(&mut self.hazards, &mut self.projectiles);
    }

    /// The Air Horn's blast from the kart in `firer`, at the fire (ADR
    /// 0076), with `in_pit_lane` saying who stands on the lane.
    fn blast_horn(&mut self, firer: usize, in_pit_lane: &[bool]) {
        let blast = HornBlast::new(
            &self.track,
            self.physics.bodies[self.karts[firer].body].translation(),
            self.karts[firer].spline_progress,
        );
        let targets: Vec<Target> = (0..self.karts.len())
            .map(|slot| self.target(slot, in_pit_lane[slot]))
            .collect();
        let resolution = blast.resolve(firer, &targets, &self.hazards, &self.projectiles);
        self.apply(&resolution);
    }

    /// Take the item box `item_box` for the kart in `slot` and start the
    /// box's respawn clock. An empty slot starts rolling an item drawn by
    /// the kart's race position as of `standings`; a slot rolling or
    /// holding one stays as it is, and nothing is drawn from the RNG.
    fn pick_up(&mut self, slot: usize, item_box: usize, standings: &[Standing]) {
        let mut item = None;
        if self.karts[slot].item_slot == ItemSlot::Empty {
            // The draw scaled down to the table's range, so the roll is as
            // even as the draw.
            let roll =
                ((u64::from(self.rng.next_u32()) * u64::from(ITEM_ROLL_PERCENT)) >> 32) as u32;
            let rolled = Item::rolled(standings[slot].position, roll);
            self.karts[slot].item_slot = ItemSlot::Rolling {
                item: rolled,
                roll_left_s: ITEM_ROLL_S,
            };
            item = Some(rolled);
        }
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
    if !matches!(state.phase, Phase::Finished) {
        state.tick += 1;
    }
    // Every kart drives this tick on the wet the last one left.
    let wetness = state.wetness();
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
        let profile = state.picks.level.profile();
        state
            .karts
            .iter_mut()
            .enumerate()
            .map(|(slot, kart)| match &mut kart.bot {
                Some(memory) => bot::drive(
                    track,
                    profile,
                    &readouts,
                    &standings,
                    clock,
                    seed,
                    kart.race_laps,
                    slot,
                    memory,
                    wetness,
                    dt,
                ),
                None => inputs[slot],
            })
            .collect()
    };
    // Race positions as of the last tick: what a projectile picks its
    // target by and a pickup rolls by.
    let standings = state.standings();
    // Who stands on the lane's safe ground as the tick begins (ADR 0081).
    let in_pit_lane: Vec<bool> = (0..state.karts.len())
        .map(|slot| state.in_pit_lane(slot))
        .collect();
    // Fires come before pickups, so a press on the tick a box is picked up
    // fires what the kart held, never what it has just rolled. A stalled or
    // spun-out kart's press is as dead as its other controls, and no item
    // can be fired in the pit lane: the kart keeps it.
    // Fires run in slot order, so of two horns pressed on one tick the
    // lower slot's blast goes first and a kart it spins out loses its press.
    for (slot, input) in inputs.iter().enumerate() {
        let kart = &mut state.karts[slot];
        let pressed = input.use_item || (input.hold_item && !kart.item_was_held);
        kart.item_was_held = input.hold_item;
        let out_of_items = kart.finished() || elapsed_s.is_none();
        if kart.ignores_inputs() || out_of_items || in_pit_lane[slot] || !pressed {
            continue;
        }
        let Some(item) = kart.item_slot.held() else {
            continue;
        };
        kart.item_slot = ItemSlot::Empty;
        // The fire is reported before what it does.
        state.events.push(RaceEvent::ItemFired { kart: slot, item });
        let kart = &mut state.karts[slot];
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
                    progress: kart.spline_progress,
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
            // The run's timer goes back to the start, never adding to
            // what was left; the kick fires again.
            Item::Star => {
                kart.star_left_s = STAR_S;
                kart.handling
                    .boost(STAR_KICK_S, STAR_KICK_FACTOR, MiniTurbo::Red);
            }
            Item::Horn => state.blast_horn(slot, &in_pit_lane),
        }
    }
    // Rolls run down after the fires, so a press on the tick a roll lands
    // fires nothing, and before the pickups, so a roll started this tick
    // has the whole tuned time to run. Nothing stops a roll early: it runs
    // through a spin-out, a respawn and the finish. A star's run runs down
    // the same way, from the tick of its fire, through all three but the
    // finish.
    for kart in &mut state.karts {
        kart.item_slot = kart.item_slot.after(dt);
        if kart.starring() {
            kart.star_left_s -= dt;
        }
    }
    // Picked-up boxes come back on their clocks; a kart standing on one
    // picks it up the tick it is back. A kart takes every box its centre is
    // on, holding an item or not, so a kart in front denies boxes to the
    // karts behind; only an empty slot rolls.
    for respawn_s in &mut state.item_box_respawn_s {
        if *respawn_s > 0.0 {
            *respawn_s -= dt;
        }
    }
    for slot in 0..state.karts.len() {
        let kart = &state.karts[slot];
        if kart.finished() || elapsed_s.is_none() {
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
    // coin its centre is on, airborne or not; with a full purse the count
    // holds but the coin is still gone, so a kart in front denies it to the
    // karts behind. A spun-out kart takes none, nor, as with boxes, a
    // finished kart or any kart through the outro.
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
            let respawn_s = &mut state.coin_respawn_s[coin];
            if *respawn_s <= 0.0 && placed.covers(position, COIN_HALF_LENGTH_M, COIN_HALF_WIDTH_M) {
                kart.coins = (kart.coins + 1).min(COIN_CAP);
                *respawn_s = COIN_RESPAWN_S;
                state
                    .events
                    .push(RaceEvent::CoinPicked { kart: slot, coin });
            }
        }
    }
    // The wakes are judged from every body as the tick begins, before any
    // kart's handling moves it, so the order the karts are ticked in never
    // decides who is in whose wake.
    let motions: Vec<(Vec3, Vec3)> = state
        .karts
        .iter()
        .map(|kart| {
            let body = &state.physics.bodies[kart.body];
            (body.translation(), body.linvel())
        })
        .collect();
    let situations: Vec<Situation> = (0..state.karts.len())
        .map(|slot| {
            let body = &state.physics.bodies[state.karts[slot].body];
            let position = body.translation().to_array();
            Situation {
                wake: state.wakes_judged.then(|| in_a_wake(&motions, slot)),
                rotation: *body.rotation(),
                vertical_m_per_s: body.linvel().y,
                surface: state.physics.surface_at(position),
                on_the_ground: state.physics.kart_on_the_ground(position),
                boost_pad: state.track.on_a_boost_pad(position),
                kerb: state.track.on_a_kerb(position),
                trick_window: state
                    .track
                    .in_a_trick_window(position, body.linvel().to_array()),
                coin_factor: state.karts[slot].coin_factor(),
                // A star's run raises the cap on top of the coins and the
                // rubber band, so the brake still works under it.
                top_speed: TOP_SPEED_M_PER_S
                    * state.rubber_band(slot)
                    * state.karts[slot].coin_factor()
                    * if state.karts[slot].starring() {
                        STAR_TOP_SPEED_FACTOR
                    } else {
                        1.0
                    },
                starring: state.karts[slot].starring(),
                pit_lane: in_pit_lane[slot],
                wetness,
            }
        })
        .collect();
    // Whether each kart holds the item button this tick, as the controls
    // reach the handling: a kart dead to its controls holds nothing.
    let mut holds = vec![false; state.karts.len()];
    for (slot, (((kart, input), situation), hold)) in state
        .karts
        .iter_mut()
        .zip(&inputs)
        .zip(situations)
        .zip(&mut holds)
        .enumerate()
    {
        // A stalled, spun-out or stopping kart sits dead to every control
        // until its time runs out; a spun-out or stopping one stands where
        // it is, whatever runs into it, and the tick its spin-out or stop
        // runs out it moves again.
        let freeze = if kart.ignores_inputs() {
            kart.stall_left_s -= dt;
            kart.spin_out_left_s -= dt;
            if kart.held_still() {
                Freeze::Stopped
            } else {
                Freeze::Controls
            }
        } else {
            Freeze::Free
        };
        // A hold ends at the entry line: no hazard is trailed into the lane.
        *hold = freeze == Freeze::Free && input.hold_item && !in_pit_lane[slot];
        // For the run the kart's collider ignores other karts, in both
        // directions; a run that ends with karts overlapping resolves as an
        // ordinary bump from here.
        state.physics.colliders[kart.collider]
            .set_collision_groups(kart_groups(kart.starring(), kart.stopping()));
        let body = &mut state.physics.bodies[kart.body];
        body.lock_translations(kart.held_still(), true);
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
        // A stopping kart is where its stop puts it: guided onto its mark and
        // held there until green, when it stands on new tyres.
        if kart.stopping() {
            hold_in_pit_box(kart, &state.track, &mut state.physics, dt);
        }
        let mut after = state.physics.bodies[kart.body].translation().to_array();
        // The tick's handling read the kart where it stood before it moved, and
        // a contact can add to what it sent: a kart that ends the tick in the
        // lane ends it at the limit, the tick it came over the line included.
        if state.track.in_pit_lane(after) {
            let body = &mut state.physics.bodies[kart.body];
            let held = kart.handling.held_to_the_pit_limit(body.linvel());
            body.set_linvel(held, true);
        }
        let mut respawned = false;
        let on_the_line = state.track.point_at(kart.spline_progress);
        // The pit wall stands between the lane and the centreline, so a kart
        // on the lane's road has not left the course for being behind it.
        let on_pit_lane = state.track.pit_lane().and_then(|lane| lane.reading(after));
        // A kart on Tyres On that comes within the catch of its pit box along
        // the lane, between the lines, is taken in for a stop (ADR 0082); on
        // Tyres Off the pit boxes do nothing. Read as a crossing of the catch, so
        // a kart let go on green, standing past it, is not taken in again.
        if let (Some(last_m), Some(now_m), Some(lane)) =
            (kart.pit_lane_m, on_pit_lane, state.track.pit_lane())
        {
            let catch_m = lane.pit_box_m(kart.slot) - PIT_BOX_CATCH_M;
            if !kart.stopping()
                && kart.handling.tyres() == Tyres::On
                && elapsed_s.is_some()
                && lane.limits(after)
                && last_m < catch_m
                && now_m >= catch_m
            {
                let (tier, stationary_s) = stop_roll(state.seed, kart.slot, kart.stops);
                kart.stop = Some(Stop {
                    tier,
                    stationary_s,
                    guide_left_s: PIT_BOX_GUIDE_S,
                    stationary_so_far_s: 0.0,
                });
            }
        }
        if state.physics.surface_at(after) == Surface::OutOfWorld
            || (on_pit_lane.is_none() && state.physics.wall_between(on_the_line, after))
        {
            kart.off_course_s += dt;
            if kart.off_course_s >= RESPAWN_DELAY_S {
                after = match (kart.pit_lane_m, state.track.pit_lane()) {
                    (Some(metres), Some(lane)) => {
                        respawn_on_lane(kart, lane, metres, &mut state.physics)
                    }
                    _ => respawn(kart, &state.track, &mut state.physics),
                };
                state.events.push(RaceEvent::Respawned { kart: index });
                // A teleport is not a move: it crosses no gate.
                before = after;
                respawned = true;
            }
        } else {
            kart.off_course_s = 0.0;
            kart.pit_lane_m = on_pit_lane;
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
                    // can change the result for the karts still racing. A
                    // star's run ends there too.
                    kart.bot.get_or_insert_with(bot::Memory::default);
                    kart.star_left_s = 0.0;
                }
            }
        }
    }
    // The whole field gets wet together, from the first kart to start the lap.
    if state.rain_began_tick.is_none() {
        if let Some(lap) = state.rain_lap {
            if state
                .karts
                .iter()
                .any(|kart| kart.laps_completed + 1 >= lap)
            {
                state.rain_began_tick = Some(state.tick);
            }
        }
    }
    // Who stands on the lane's safe ground now the karts have moved: no item
    // or star touches them from here on (ADR 0081).
    let in_pit_lane: Vec<bool> = (0..state.karts.len())
        .map(|slot| state.in_pit_lane(slot))
        .collect();
    let targets = (0..state.karts.len())
        .map(|slot| state.target(slot, in_pit_lane[slot]))
        .collect();
    let wheels: Vec<(u32, [f32; 3])> = match elapsed_s {
        Some(elapsed_s) => (0..state.track.wheel_crossings().len())
            .filter_map(
                |crossing| match wheel(&state.track, crossing, state.seed, elapsed_s) {
                    Wheel::Rolling { pass, position, .. } => Some((pass, position)),
                    _ => None,
                },
            )
            .collect(),
        None => Vec::new(),
    };
    let resolution = hits::resolve(
        &state.track,
        targets,
        &holds,
        &wheels,
        &mut state.hazards,
        &mut state.projectiles,
        dt,
    );
    state.apply(&resolution);
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
/// level's chance. The draw is made at every level, so the RNG runs on the
/// same from one seed whatever the level; a chance of one always rolls it.
fn rolls_the_start_boost(rng: &mut ChaCha8Rng, level: BotLevel) -> bool {
    let chance = level.profile().start_boost_chance;
    let draw = f64::from(rng.next_u32()) / f64::from(u32::MAX);
    chance >= 1.0 || draw < f64::from(chance)
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
/// it with, and costs no tyre life: wear from another kart is the
/// snapshots' word (ADR 0080). Each kart of a pair runs this for itself.
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
        let other_slot = karts.iter().position(|kart| kart.body == other_body);
        let other_sent = other_slot.map_or(physics.bodies[other_body].linvel(), |slot| {
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
            if other_slot.is_some() {
                karts[index].handling.wear_for_touch(other, closing_speed);
            }
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
            kart.handling.wear_for_touch(other, closing_speed);
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
    respawn_at(kart, here, next, physics)
}

/// Run the kart's stop one tick of `dt`: over the guide it slides onto its
/// mark, the pit box of its slot, turning to face down the lane; on the
/// mark it stands, the crew at work, until the rolled time is up. Then it
/// is let go on green, at rest on new tyres, its stop counted. The mark is
/// no checkpoint: the lap clock runs on through the stop.
fn hold_in_pit_box(kart: &mut Kart, track: &Track, physics: &mut Physics, dt: f32) {
    let lane = track
        .pit_lane()
        .expect("a kart stops only on a course with a pit lane");
    let mark = lane.pit_boxes()[kart.slot];
    let on_the_mark = Vec3::from(mark.position) + Vec3::Y * KART_HALF_HEIGHT_M;
    let facing = Rotation::from_rotation_y(mark.yaw);
    let stop = kart.stop.as_mut().expect("the kart is stopping");
    let body = &mut physics.bodies[kart.body];
    if stop.guide_left_s > 0.0 {
        // The last tick of the guide lands exactly on the mark.
        let share = (dt / stop.guide_left_s).min(1.0);
        body.set_translation(body.translation().lerp(on_the_mark, share), true);
        body.set_rotation(body.rotation().lerp(facing, share).normalize(), true);
        stop.guide_left_s = if share < 1.0 {
            stop.guide_left_s - dt
        } else {
            0.0
        };
    } else {
        body.set_translation(on_the_mark, true);
        body.set_rotation(facing, true);
        stop.stationary_so_far_s += dt;
    }
    body.set_linvel(Vec3::ZERO, true);
    body.set_angvel(Vec3::ZERO, true);
    if stop.stationary_so_far_s < stop.stationary_s {
        return;
    }
    let stationary_s = stop.stationary_s;
    kart.stop = None;
    kart.stops += 1;
    kart.best_stop_s = Some(
        kart.best_stop_s
            .map_or(stationary_s, |best| best.min(stationary_s)),
    );
    kart.handling.new_tyres();
    kart.handling.respawn(facing * Vec3::NEG_Z);
}

/// Put a kart that left the pit lane's road back on it, `metres` along it,
/// where it last was, facing down it. It gains nothing: the lane is the
/// slower road and it crosses no gate. Returns where it now stands.
fn respawn_on_lane(
    kart: &mut Kart,
    lane: &track::PitLane,
    metres: f32,
    physics: &mut Physics,
) -> [f32; 3] {
    // A metre of the path is left ahead, for the kart to read which way it faces.
    let metres = metres.min(lane.length_m() - 1.0);
    let here = lane.point_at(metres);
    let next = lane.point_at(metres + 1.0);
    respawn_at(kart, here, next, physics)
}

/// Stand the kart at `here` at rest, facing `next`, with nothing carried
/// over from before it fell. Returns where it now stands.
fn respawn_at(kart: &mut Kart, here: [f32; 3], next: [f32; 3], physics: &mut Physics) -> [f32; 3] {
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
        if kart.laps_completed >= kart.race_laps {
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

/// Stop the kart in `slot` where it is and start its spin-out, `by` the
/// kart whose item or star hit it or the runaway wheel: a drift or a boost ends, a
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
    kart.handling.wear_for_spin_out();
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
    /// centreline, and `kart`, seen level at a kart's height over the line.
    /// A kart behind a wall has left the course, since no way back crosses
    /// one; a kart held against a wall from the run-off is not behind it.
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
