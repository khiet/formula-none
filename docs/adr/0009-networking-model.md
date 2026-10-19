# ADR 0009: Networking model

## Status

Accepted (2026-10-18). The input handling in its Outcome is superseded by
ADR 0019.

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

Prediction landed with issue 18 (2026-10-19), together with the server
input queue of ADR 0019, which replaces the input handling recorded above.

- The client predicts its own kart with `sim::Prediction`: the sim's
  `step` on a race of that one kart on the static track, through the
  bridge, so prediction runs the server's rules and nothing else. It lays
  its inputs out on the server's ticks by the lead the snapshots
  acknowledge (ADR 0019).
- A snapshot that agrees with the prediction at its tick, to the tuned
  millimetre, changes nothing. When one does not, the prediction rewinds to
  it and replays the later inputs. The jump is counted and blended out over
  `RECONCILIATION_BLEND_TICKS`: six sim ticks, the six frames above at the
  sim's rate, the same on any display. Snapshot fields the prediction does
  not read stay its own, because it ran the same rules on the same inputs.
- The fake-transport test holds the own kart's corrections under
  `RECONCILIATION_VISIBLE_CORRECTION_M` (0.25 m) at 100 ms RTT with 2
  percent loss. It leaves out snapshots after the kart has touched another,
  because kart contact is the server's alone until the other karts are
  local bodies (issue 19).
- The Instrument panel's countdown and race clock are the prediction's: the
  clock at the tick the player's inputs meet on the server.

Interpolation landed with issue 19 (2026-10-19).

- The client draws the other karts with `sim::Interpolation`, between the
  two snapshots it holds either side of a render clock
  `INTERPOLATION_DELAY_S` behind the newest. The clock runs a tick per
  client tick, nudged toward the delay by at most
  `INTERPOLATION_CLOCK_SLEW` of a tick, and never past the newest snapshot
  held, so it reads no snapshot the client has not received. A kart that
  moved further between two snapshots than any kart drives was respawned,
  and is drawn where it was until the snapshot that has it in its new
  place.
- The prediction places the other karts where the client draws them, as
  kinematic bodies, every tick it predicts and every tick it replays. The
  own kart meets them and cannot push them.
- The fake-transport tests hold the render clock at or behind the newest
  snapshot at up to 50 ms jitter, 100 ms behind it on average. At 100 ms
  RTT with 2 percent loss, they hold every kart within
  `RECONCILIATION_VISIBLE_CORRECTION_M` of where it is drawn without loss.
  The prediction test of issue 18 no longer leaves out snapshots after a
  touch.
- Driving the own kart into another kart leaves a few centimetres of
  overrun to correct once the server pushes the other kart on: it is
  drawn 100 ms behind, overlapping the own kart, and the contact pushes
  the own kart out ahead of it. With no lag compensation, the test holds
  that under `RECONCILIATION_VISIBLE_CORRECTION_M`. Without the kinematic
  bodies it is most of a metre.
