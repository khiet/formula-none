# ADR 0026: One kart readout across the wasm and wire seams

## Status

Accepted (2026-10-25). Supersedes the consequence of ADR 0003 that a value
the renderer needs extends the flat buffer, and the one-way rule of
ADR 0006 where it kept protocol off sim.

## Context

A kart's state had five shapes, and every per-kart field of a snapshot was
copied by hand between them: the sim's `KartTransform` and `SnapshotKart`,
the protocol's `KartSnapshot`, and the bridge's eighteen-float row decoded
column by column in `bridge.ts`. Adding one field touched about ten files.
The prediction rebuilt the sim's own derived state (travel, speed, the
boost clock, laps completed) from a snapshot, away from the handling that
produces it. The server and the bridge each had their own copy of the wire
conversions, and the server's tests a third.

ADR 0003 chose a flat buffer so Three.js reads the karts' transforms
without crossing the wasm boundary per kart per field. That reason holds
for the seven transform floats the View reads every frame. It never held
for the rest of the row.

## Decision

- The sim exports one per-kart readout, `KartReadout` (GLOSSARY: kart
  readout). `State::kart_readouts` produces one per kart every tick, the
  prediction and the interpolation produce the same type, and a kart
  adopts one from a snapshot beside the handling that reads it.
- The bridge carries the transform, position then rotation, as a flat
  buffer of seven floats a kart. The rest of each readout, the surface
  under the kart and each kart's standing cross as JavaScript values
  through `serde_wasm_bindgen`.
- The readout's TypeScript type, with the sim types it names, the
  `Standing`, and the `Tuning` the bridge hands over in one `tuning()`
  call, is generated with ts-rs into `apps/web/src/generated/sim` by the
  protocol's generator (ADR 0004). CI regenerates it and fails on a diff.
- `protocol` depends on `sim` and holds the conversions between the wire's
  types and the sim's, one `From` each way. The orphan rule allows these
  impls only in a crate that owns one of the types. The server, the bridge
  and the server's tests all use them.

## Consequences

- A field the sim adds to the readout reaches the web app's type by
  regenerating, with no column to assign on either side of the seam. A
  field the snapshot should carry is added to `KartSnapshot` and to its
  two conversions in `protocol`.
- The dependency rule is now: the web app depends on the bridge; the bridge
  and the server depend on sim and protocol; protocol depends on sim; sim
  depends on nothing in the repo. The repo-checks tests enforce it.
- The sim depends on ts-rs for the derive. The `TS` impls are dead code in
  the wasm build.
- A sim value crosses to JavaScript with a `usize` as a number, as ts-rs
  types it; a message keeps its `u64` as a bigint.
- The phase, item boxes, hazards and projectiles still cross as flat
  buffers. They are not per-kart state, and moving them is a separate
  decision.
