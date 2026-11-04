//! The race distance: how many laps a race runs (ADR 0083).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::tuning::{GRAND_PRIX_LAPS, LAPS_PER_RACE};

/// How long a race runs: the creator's pick, fixed for the room as its course
/// is, and carried by a replay. A Sprint is what a kept room or a replay
/// without a pick comes back as, whatever a later default becomes (ADR 0083).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum RaceDistance {
    #[default]
    Sprint,
    GrandPrix,
}

impl RaceDistance {
    /// Every pick, the Sprint first.
    pub const ALL: [Self; 2] = [Self::Sprint, Self::GrandPrix];

    /// The pick as the wire spells it: `sprint` or `grand-prix`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sprint => "sprint",
            Self::GrandPrix => "grand-prix",
        }
    }

    /// The pick spelled `name` on the wire, or `None` for any other spelling,
    /// so a caller that takes the pick as text refuses an unknown one rather
    /// than racing a Sprint.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|distance| distance.name() == name)
    }

    /// The laps a kart completes to finish a race of this distance.
    pub fn laps(self) -> u32 {
        match self {
            Self::Sprint => LAPS_PER_RACE,
            Self::GrandPrix => GRAND_PRIX_LAPS,
        }
    }
}
