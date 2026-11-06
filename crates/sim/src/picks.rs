//! The creator's picks: everything about how a race runs that the creator
//! chooses on the Title Scene and the room then fixes, the course apart.
//! One value carries them from the Title Scene to the sim, so a pick added
//! here and in [`crate::tuning`] travels the bridge, the wire, the room and
//! a replay without any of them naming it.

use crate::tuning::{DEFAULT_LAPS, PICK_CHOICES};
use crate::{BotLevel, Tyres};
use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// The bot level (ADR 0078), the Tyres pick (ADR 0083) and the laps
/// (ADR 0084). A field left out of a value read back, a kept room or a
/// replay from before its pick existed, takes [`Picks::default`]: the race
/// as it was before there was a pick, whatever a creator is now offered
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(default, rename_all = "camelCase")]
pub struct Picks {
    pub level: BotLevel,
    pub tyres: Tyres,
    pub laps: u32,
}

impl Default for Picks {
    fn default() -> Self {
        Self {
            level: BotLevel::Basic,
            tyres: Tyres::Off,
            laps: DEFAULT_LAPS,
        }
    }
}

impl Picks {
    /// These picks, or the first one no creator can make. Every caller that
    /// takes picks from outside the sim asks this before racing them, so an
    /// unknown pick is refused rather than raced.
    pub fn offered(self) -> Result<Self, UnofferedPick> {
        if !PICK_CHOICES.laps.contains(&self.laps) {
            return Err(UnofferedPick::Laps(self.laps));
        }
        Ok(self)
    }
}

/// A pick outside [`PICK_CHOICES`]. The enums cannot be one: decoding
/// refuses a name they do not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnofferedPick {
    Laps(u32),
}

impl fmt::Display for UnofferedPick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Laps(laps) => write!(f, "{laps} laps, which no race can run"),
        }
    }
}

impl std::error::Error for UnofferedPick {}

/// What a creator can pick from, each list in the Title Scene's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase")]
pub struct PickChoices {
    pub level: [BotLevel; 3],
    pub tyres: [Tyres; 2],
    pub laps: [u32; 2],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::{FRESH_PICKS, LAP_CHOICES};

    #[test]
    fn every_offered_lap_count_is_offered_and_no_other() {
        for laps in LAP_CHOICES {
            let picks = Picks {
                laps,
                ..Picks::default()
            };
            assert_eq!(picks.offered(), Ok(picks));
        }
        let four = Picks {
            laps: 4,
            ..Picks::default()
        };
        assert_eq!(four.offered(), Err(UnofferedPick::Laps(4)));
    }

    #[test]
    fn the_fresh_picks_are_offered() {
        assert_eq!(FRESH_PICKS.offered(), Ok(FRESH_PICKS));
    }

    #[test]
    fn a_value_missing_a_pick_reads_it_as_before_there_was_one() {
        let picks: Picks = serde_json::from_str(r#"{ "level": "ace" }"#).unwrap();
        assert_eq!(
            picks,
            Picks {
                level: BotLevel::Ace,
                ..Picks::default()
            }
        );
    }
}
