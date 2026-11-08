//! The weather (ADR 0103): whether it rains, on which lap, and how wet that
//! leaves the track. What wetness does to a kart is [`TyreGrip::wet`]'s.
//!
//! [`TyreGrip::wet`]: crate::handling::TyreGrip::wet

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::hash;
use crate::tuning::RAIN_WETTING_TICKS;

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

/// The lap a race of `laps` from `seed` turns wet on: from lap 2 to the lap
/// before the last, or lap 2 or 3 of a three-lap race, which would otherwise
/// have one lap to choose from. A hash, so it draws nothing from the race's
/// random stream.
pub fn rain_lap(seed: u64, laps: u32) -> u32 {
    const RAIN_LAP_SALT: u64 = 0x5241_494e_4c41_5000;
    let last = laps.saturating_sub(1).max(3);
    let choices = last - 1;
    2 + (hash::unit(seed ^ RAIN_LAP_SALT, u64::from(laps)) * choices as f32) as u32
}

/// How wet the track is at `tick` for rain that began at `began`: from 0 on
/// that tick to 1 over the tuned time, then held. Whole ticks over whole
/// ticks, so a tick before `began`, as a rewind can replay, is simply dry.
pub(crate) fn wetness(began: u32, tick: u32) -> f32 {
    tick.saturating_sub(began).min(RAIN_WETTING_TICKS) as f32 / RAIN_WETTING_TICKS as f32
}
