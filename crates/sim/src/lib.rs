//! The race simulation: one deterministic `step` shared by the browser (via
//! the `bridge` crate) and the server. No I/O, no async runtime, nothing else
//! from this repo.

pub mod tuning;
