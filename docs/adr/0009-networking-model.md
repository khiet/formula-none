# ADR 0009: Networking model

## Status

Accepted (2026-10-18)

## Context

The player's own kart must respond instantly while the server stays the
authority; other karts must move smoothly over real internet latency.

## Decision

- Server-authoritative. The client predicts only its own kart against the
  static track.
- Reconciliation: on each snapshot the client rewinds to the snapshot tick
  and replays unacknowledged inputs; the visible correction is blended over
  up to six frames.
- Other karts are rendered by interpolating between snapshots 100 ms behind
  the newest and are treated as kinematic bodies for local collision.
- Kart-to-kart contact is resolved on the server only. No lag compensation.
- Cheating is out of scope beyond server authority and the sim's own input
  clamping.

## Consequences

- Prediction shares code with authority (ADR 0002), so correction magnitude
  measures network conditions, not rule divergence.
- Interpolation must never read a future snapshot; the fake-transport tests
  assert this.
- Target: no visible correction of your own kart at 100 ms RTT with 2
  percent loss.
