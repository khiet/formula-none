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

## Outcome

Server authority landed with issue 17 (2026-10-19); prediction (issue 18)
and interpolation (issue 19) follow.

- The server applies the newest input sequence each human has sent and
  holds it until a newer one arrives, so a lost or reordered datagram
  changes nothing; each snapshot acknowledges the sequence applied per
  kart.
- Until prediction lands, every client draws every kart, its own included,
  where the newest snapshot puts it (`apps/web/src/server-race.ts`); a
  snapshot older than the newest seen is dropped.

Prediction landed with issue 18 (2026-10-19), and with it a change to how
the server takes inputs: holding the newest skipped one input and repeated
another whenever a datagram landed either side of a tick, and no
prediction can follow a skip it cannot see.

- The server applies one sequence per tick, in order, once it holds the
  tuned two (`crates/server/src/race.rs`), so the tick an input is applied
  at is fixed by its sequence. A sequence that has not arrived by its tick
  is applied as the last controls and is not applied later. The server
  waits for a client that falls the tuned three ticks behind it and drops
  inputs from one the same distance ahead of it.
- A snapshot acknowledges the sequence applied at its tick, so sequence
  A + n will be applied at tick T + n; the client lays its inputs out on
  the server's ticks by that lead.
- The client predicts its own kart with `sim::Prediction`: the sim's
  `step` on a race of that one kart on the static track, through the
  bridge, so prediction runs the server's rules and nothing else. A
  snapshot that agrees with the prediction at its tick to the tuned
  millimetre changes nothing; one that does not is rewound to and the
  later inputs replayed, and the jump is counted and blended out over the
  tuned six frames. Snapshot fields the prediction does not read stay its
  own: they ran the same rules on the same inputs.
- The fake-transport test holds the own kart's corrections under
  `RECONCILIATION_VISIBLE_CORRECTION_M` (0.25 m) at 100 ms RTT with 2
  percent loss, leaving out snapshots after the kart has touched another:
  kart contact is the server's alone until the other karts are local
  bodies (issue 19).
- The Instrument panel's countdown and race clock are the prediction's,
  the clock at the tick the player's inputs meet on the server.
