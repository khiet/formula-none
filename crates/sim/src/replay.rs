//! A race replay: `{seed, track, bots, level, inputs[]}`, enough to rerun a
//! race anywhere (ADR 0002). Bots drive themselves from the seed at the
//! race's bot level, so a replay carries how many there were, the level,
//! and only the humans' inputs. Rerunning
//! reports [`State::hash`] at every [`REPLAY_HASH_INTERVAL_TICKS`]th tick; a
//! fixture carries the hashes a previous run reported so a later run, native
//! or wasm, can be held to them.
//!
//! The JSON form is the one the web app records and the fixtures under
//! `assets/replays` use:
//!
//! ```json
//! {
//!   "version": 1,
//!   "seed": "7",
//!   "track": "oval",
//!   "bots": 0,
//!   "level": "basic",
//!   "segments": [
//!     {
//!       "ticks": 90,
//!       "inputs": [
//!         {
//!           "steer": 0.0, "throttle": 1.0, "brake": 0.0,
//!           "hop": false, "useItem": false, "holdItem": false
//!         }
//!       ]
//!     }
//!   ],
//!   "hashes": ["3e0ad4e84a0b22c7"]
//! }
//! ```
//!
//! Inputs are run-length encoded: a segment holds one input per human kart
//! in grid order and the number of consecutive ticks it was held. A missing
//! button reads as unpressed, a missing bot count as none and a missing
//! level as Basic, so a replay recorded before any of them existed still
//! runs. The seed and the hashes are
//! 64-bit values, which JSON numbers cannot carry exactly, so the seed is
//! written as a decimal string and each hash as 16 hex digits.

use crate::tuning::REPLAY_HASH_INTERVAL_TICKS;
use crate::{step, BotLevel, KartInput, State, Track};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The only replay format this build reads and writes.
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Replay {
    pub seed: u64,
    /// The track's name as its sidecar gives it.
    pub track: String,
    /// Bot karts on the grid behind the humans; they need no inputs.
    pub bots: usize,
    /// How hard the bots drive (ADR 0078).
    pub level: BotLevel,
    pub segments: Vec<Segment>,
    /// [`State::hash`] after every [`REPLAY_HASH_INTERVAL_TICKS`]th tick, as
    /// recorded. Empty until a run has been recorded against the replay.
    pub hashes: Vec<u64>,
}

/// The JSON form: a [`Replay`] plus the format version, so a file from a
/// later format fails to parse instead of replaying wrongly.
#[derive(Serialize, Deserialize)]
struct ReplayFile {
    version: u32,
    #[serde(with = "decimal_seed")]
    seed: u64,
    track: String,
    #[serde(default)]
    bots: usize,
    #[serde(default)]
    level: BotLevel,
    segments: Vec<Segment>,
    #[serde(with = "hex_hashes")]
    hashes: Vec<u64>,
}

/// One input per human kart in grid order, held for `ticks` consecutive
/// ticks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub ticks: u32,
    pub inputs: Vec<KartInput>,
}

#[derive(Debug)]
pub enum ReplayError {
    Json(serde_json::Error),
    /// The file is from another format version than this build reads.
    Version(u32),
    /// The replay was recorded on one track and run on another.
    WrongTrack {
        expected: String,
        got: String,
    },
    /// The first segment's humans and the bots make this many karts, but the
    /// track's grid has fewer slots.
    TooManyKarts {
        karts: usize,
        slots: usize,
    },
    /// A later segment has a different kart count from the first.
    UnevenSegments {
        segment: usize,
        karts: usize,
        first: usize,
    },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "replay is not valid: {error}"),
            Self::Version(version) => {
                write!(
                    f,
                    "replay is version {version} but this build reads version {VERSION}"
                )
            }
            Self::WrongTrack { expected, got } => {
                write!(f, "replay was recorded on {expected} but run on {got}")
            }
            Self::TooManyKarts { karts, slots } => {
                write!(
                    f,
                    "replay has {karts} karts but the track has {slots} spawn slots"
                )
            }
            Self::UnevenSegments {
                segment,
                karts,
                first,
            } => write!(
                f,
                "segment {segment} has {karts} karts but the first segment has {first}"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<serde_json::Error> for ReplayError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl Replay {
    pub fn from_json(json: &str) -> Result<Self, ReplayError> {
        let wire: ReplayFile = serde_json::from_str(json)?;
        if wire.version != VERSION {
            return Err(ReplayError::Version(wire.version));
        }
        Ok(Self {
            seed: wire.seed,
            track: wire.track,
            bots: wire.bots,
            level: wire.level,
            segments: wire.segments,
            hashes: wire.hashes,
        })
    }

    /// Pretty-printed, so a fixture diffs by segment.
    pub fn to_json(&self) -> String {
        let wire = ReplayFile {
            version: VERSION,
            seed: self.seed,
            track: self.track.clone(),
            bots: self.bots,
            level: self.level,
            segments: self.segments.clone(),
            hashes: self.hashes.clone(),
        };
        serde_json::to_string_pretty(&wire).expect("a replay has no unserialisable field")
    }

    /// Human karts, as the first segment has inputs; zero for an empty
    /// replay.
    pub fn human_count(&self) -> usize {
        self.segments
            .first()
            .map_or(0, |segment| segment.inputs.len())
    }

    pub fn tick_count(&self) -> u32 {
        self.segments.iter().map(|segment| segment.ticks).sum()
    }

    /// Rerun the replay on `track`, which must be the track it names, and
    /// report the state hash after every [`REPLAY_HASH_INTERVAL_TICKS`]th
    /// tick. Ticks past the last whole interval run but are not hashed.
    pub fn run(&self, track: Track) -> Result<Vec<u64>, ReplayError> {
        if track.name() != self.track {
            return Err(ReplayError::WrongTrack {
                expected: self.track.clone(),
                got: track.name().to_owned(),
            });
        }
        let humans = self.human_count();
        if humans + self.bots > track.spawn_grid().len() {
            return Err(ReplayError::TooManyKarts {
                karts: humans + self.bots,
                slots: track.spawn_grid().len(),
            });
        }
        if let Some((segment, uneven)) = self
            .segments
            .iter()
            .enumerate()
            .find(|(_, segment)| segment.inputs.len() != humans)
        {
            return Err(ReplayError::UnevenSegments {
                segment,
                karts: uneven.inputs.len(),
                first: humans,
            });
        }

        let dt = 1.0 / crate::tuning::TICKS_PER_SECOND as f32;
        let mut state = State::with_bots(self.seed, humans, self.bots, track, self.level);
        let mut hashes =
            Vec::with_capacity((self.tick_count() / REPLAY_HASH_INTERVAL_TICKS) as usize);
        let mut tick = 0;
        for segment in &self.segments {
            for _ in 0..segment.ticks {
                state = step(state, &segment.inputs, dt);
                tick += 1;
                if tick % REPLAY_HASH_INTERVAL_TICKS == 0 {
                    hashes.push(state.hash());
                }
            }
        }
        Ok(hashes)
    }
}

mod decimal_seed {
    use serde::de::{Deserializer, Error};
    use serde::{Deserialize, Serializer};

    pub fn serialize<S: Serializer>(seed: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&seed.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let seed = String::deserialize(deserializer)?;
        seed.parse()
            .map_err(|_| D::Error::custom(format!("seed {seed:?} is not a decimal u64")))
    }
}

mod hex_hashes {
    use serde::de::{Deserializer, Error};
    use serde::{Deserialize, Serialize, Serializer};

    pub fn serialize<S: Serializer>(hashes: &[u64], serializer: S) -> Result<S::Ok, S::Error> {
        hashes
            .iter()
            .map(|hash| format!("{hash:016x}"))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u64>, D::Error> {
        Vec::<String>::deserialize(deserializer)?
            .iter()
            .map(|hex| {
                u64::from_str_radix(hex, 16)
                    .map_err(|_| D::Error::custom(format!("hash {hex:?} is not 16 hex digits")))
            })
            .collect()
    }
}
