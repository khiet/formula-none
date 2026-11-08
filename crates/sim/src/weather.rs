//! The weather (ADR 0103): whether it rains, on which lap, and how wet that
//! leaves the track. What wetness does to a kart is [`TyreGrip::wet`]'s.
//!
//! [`TyreGrip::wet`]: crate::handling::TyreGrip::wet

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::hash;
use crate::tuning::{RAIN_FIRST_LAP, RAIN_LAST_LAP_AT_LEAST, RAIN_WETTING_TICKS};

/// Whether rain comes in this race: the creator's pick, fixed for the room as
/// its course is, and carried by a replay. Dry is the race as it was before
/// there was weather, and what a kept room or a replay without a pick comes
/// back as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "kebab-case")]
pub enum Weather {
    #[default]
    Dry,
    Rain,
}

impl Weather {
    /// Every pick, Dry first.
    pub const ALL: [Self; 2] = [Self::Dry, Self::Rain];
}

/// The laps the seed can pick the rain's from in a race of `laps`.
pub fn rain_laps(laps: u32) -> RangeInclusive<u32> {
    RAIN_FIRST_LAP..=laps.saturating_sub(1).max(RAIN_LAST_LAP_AT_LEAST)
}

/// The lap a race of `laps` from `seed` turns wet on, one of [`rain_laps`]. A
/// hash, so it draws nothing from the race's random stream.
pub fn rain_lap(seed: u64, laps: u32) -> u32 {
    const RAIN_LAP_SALT: u64 = 0x5241_494e_4c41_5000;
    let choices = rain_laps(laps).count() as f32;
    let draw = hash::unit(seed ^ RAIN_LAP_SALT, u64::from(laps));
    RAIN_FIRST_LAP + (draw * choices) as u32
}

/// How wet the track is at `tick` for rain that began at `began`: from 0 on
/// that tick to 1 over the tuned time, then held. A tick before `began`, as a
/// rewind can replay, is dry.
pub(crate) fn wetness(began: u32, tick: u32) -> f32 {
    tick.saturating_sub(began).min(RAIN_WETTING_TICKS) as f32 / RAIN_WETTING_TICKS as f32
}
