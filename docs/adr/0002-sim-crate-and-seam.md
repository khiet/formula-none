# ADR 0002: Sim crate and its seam

## Status

Accepted (2026-10-18). The consequence that tests never reach into
internals is superseded for the handling module by ADR 0027.

## Context

Client-side prediction and server authority must never disagree on the rules.
The only way to guarantee that is to run the same code in both places, which
means the simulation can have no dependency on where it runs.

## Decision

- One Rust crate, `sim`, holds every rule of the game. It uses Rapier 3D with
  the `enhanced-determinism` feature, no tokio, no browser APIs, no I/O, and
  nothing else from this repo.
- The public seam is `step(state, inputs, dt) -> state`, deterministic given
  the seeded RNG held in `state`.
- Bots live inside the sim. They consume the same RNG and produce the same
  input struct as humans, so a bot-driven kart is indistinguishable to the
  rest of the sim.
- Determinism is a CI check: a race is `{seed, track, inputs[]}`, and the
  native and WebAssembly replays of the same fixture must hash identically at
  every 60th tick.

## Consequences

- Tests drive the sim through `step` with a seeded RNG and assert observable
  state; they never reach into internals.
- Floating-point behaviour that differs between native and wasm is a bug in
  this crate's dependency choices, caught by the replay hash.
- Anything that needs wall-clock time, randomness from the OS, or network
  belongs in the server or the bridge, not here.
