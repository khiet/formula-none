# ADR 0027: The handling as an internal seam

## Status

Accepted (2026-10-25). Supersedes the consequence of ADR 0002 that tests
never reach into the sim's internals, for the handling module only.

## Context

The handling (GLOSSARY) is one rule over a kart's speed, hop, drift,
mini-turbo, boosts, surface cap, freeze, wall hold and bump. Through
`step` alone, a test of any of them needs a track sidecar, Rapier, the
countdown held and a tick-exact key script: `drift.rs` and `surfaces.rs`
run to 460 and 700 lines mostly of that setup. The rule itself reads
nothing of the physics world or the track, so it can be driven for N ticks
without either once it has a module of its own.

## Decision

- The handling lives in `crates/sim/src/handling.rs`, one `Handling` struct
  per kart with one per-tick function, `Handling::tick`, over the struct,
  the kart's input, its freeze, what lies under it and `dt`, returning the
  velocity the solver takes. `step` runs it for every kart, solves, then
  hands each kart's handling the walls and karts it met.
- That module's interface is an internal seam: unit tests in the module
  drive it directly, with no `Track` and no physics world, and may read the
  struct's fields.
- `step` stays the public seam (ADR 0002). Every other test still drives
  the sim through it, and the integration tests under `crates/sim/tests`
  still cover the handling end to end.

## Consequences

- A handling rule is tested in a few lines at the seam that owns it; a test
  that needs the track or other karts still goes through `step`.
- The handling tests are coupled to the struct's fields, so reshaping the
  struct rewrites them; they are the module's tests and change with it.
- No other module gains this licence without a record of its own.
