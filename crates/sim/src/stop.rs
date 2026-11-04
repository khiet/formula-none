//! The stop (GLOSSARY, ADR 0082): a kart's time stationary in its pit box
//! for new tyres. Its length is rolled from the race seed, the kart's grid
//! slot and how many stops the kart has made, the way the runaway wheel
//! draws its landings from the seed: an integer hash, so the server, a
//! prediction and a replay roll the same stop and nothing is drawn from
//! the race's item generator. Pure chance: the driver only waits for green.

use serde::Serialize;
use ts_rs::TS;

use crate::tuning::{STOP_ROLL_CHANCES, STOP_ROLL_STATIONARY_S};

/// How a stop went, in the order of the roll's table: what its time says
/// and what colours the final time on the Instrument panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StopTier {
    /// Everything clicks.
    Record,
    /// A normal stop.
    Clean,
    /// A gunner goes back to one corner.
    Slow,
    /// A stuck wheel nut.
    Botched,
    /// The jack fails and the kart waits.
    Disaster,
}

impl StopTier {
    /// Every tier, in the order of the roll's table.
    pub const ALL: [Self; 5] = [
        Self::Record,
        Self::Clean,
        Self::Slow,
        Self::Botched,
        Self::Disaster,
    ];

    /// The tier as one byte in table order, for the hash and the bridge.
    pub fn code(self) -> u8 {
        Self::ALL
            .iter()
            .position(|&tier| tier == self)
            .expect("every tier is in ALL") as u8
    }
}

/// A kart's stop in its pit box, as the readout carries it: from the tick
/// the kart is taken onto its mark to the tick of green.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    /// The roll's tier, which the stationary time was drawn in.
    pub tier: StopTier,
    /// The stationary time rolled: seconds from the mark to green.
    pub stationary_s: f32,
    /// Seconds of the guide onto the mark left; zero once the kart stands
    /// on it.
    pub guide_left_s: f32,
    /// Seconds the kart has stood on its mark, the crew at work: the timer
    /// the panel counts up, which reaches `stationary_s` at green.
    pub stationary_so_far_s: f32,
}

impl Stop {
    /// Whether the kart stands on its mark with the crew at work.
    pub fn on_the_mark(&self) -> bool {
        self.guide_left_s <= 0.0
    }
}

/// The roll for the stop numbered `stops` (0 for a kart's first) of the kart
/// on grid slot `slot` in a race from `seed`: its tier, and its stationary
/// time within that tier's range. The same three numbers always roll the
/// same stop.
pub fn stop_roll(seed: u64, slot: usize, stops: u32) -> (StopTier, f32) {
    let tier_draw = unit(seed, slot, stops, 0);
    let mut sum = 0.0;
    let mut tier = StopTier::Disaster;
    for (chance, candidate) in STOP_ROLL_CHANCES.into_iter().zip(StopTier::ALL) {
        sum += chance;
        if tier_draw < sum {
            tier = candidate;
            break;
        }
    }
    let [from, to] = STOP_ROLL_STATIONARY_S[usize::from(tier.code())];
    let stationary_s = from + (to - from) * unit(seed, slot, stops, 1);
    (tier, stationary_s)
}

/// A number in [0, 1) that only `seed`, `slot`, `stops` and `draw` decide:
/// an integer hash, so it is the same natively and in wasm and draws
/// nothing from the race's item generator. The runaway wheel's, salted so
/// a stop never repeats a landing.
fn unit(seed: u64, slot: usize, stops: u32, draw: u32) -> f32 {
    const STOP_SALT: u64 = 0x5354_4f50_0000_0000;
    let mixed = (slot as u64) << 48 | u64::from(stops) << 16 | u64::from(draw);
    let mut hash = seed ^ STOP_SALT ^ mixed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // The SplitMix64 finaliser.
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 31;
    // The top 24 bits, which an f32 holds exactly.
    (hash >> 40) as f32 / (1u32 << 24) as f32
}
