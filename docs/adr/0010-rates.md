# ADR 0010: Rates

## Status

Accepted (2026-10-18)

## Context

Tick, input and snapshot rates set the feel, the bandwidth and the server
load together, so they are decided once.

## Decision

- Sim tick 60 Hz on both client and server.
- Client input messages at 60 Hz carrying sequence, steer, throttle, brake,
  hop, use item, hold item: about 8 bytes.
- Server snapshots at 30 Hz carrying tick, last acknowledged input sequence
  per client, and for every kart position, rotation, velocity, drift state
  and charge, lap, checkpoint, spline progress, held item, spin-out timer,
  plus events since the previous snapshot (item picked, item fired, hit, lap
  completed, finished, respawned). Roughly 400 bytes per snapshot at eight
  karts.

## Consequences

- Every rule is written per tick; frame rate never enters the sim.
- Snapshot size is a budget: a new per-kart field costs eight times its size
  thirty times a second.
- Delta compression is deferred to v1.1.
