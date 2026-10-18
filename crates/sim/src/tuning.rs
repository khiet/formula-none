//! Single home for gameplay, item, bot and network constants (ADR 0014).
//! Tuning a value means editing this file and nothing else; no other module
//! may hard-code a number that belongs here. Initial values are the PRD's.

/// Fixed sim rate on both client and server. Every rule is written per tick.
pub const TICKS_PER_SECOND: u32 = 60;

/// Karts on the grid; empty slots are filled by bots at Start.
pub const KARTS_PER_RACE: usize = 8;

/// Laps to finish a race.
pub const LAPS_PER_RACE: u32 = 3;

/// Top speed of a kart in metres per second. Boosts and surfaces scale it.
pub const TOP_SPEED_M_PER_S: f32 = 30.0;
