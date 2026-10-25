# ADR 0029: Standings sorted on demand

## Status

Accepted (2026-10-25). Leaves ADR 0022's lap rule as it is.

## Context

The architecture review of 2026-10-25 (issue 90, candidate 5) proposed a
progress module: a progress value on `Kart`, one module owning the
checkpoint crossing rule and the ordering, and `step` computing the
standings once per tick and storing them for the server and the bridge.

Against the code after issue 88 reshaped `step`, the review found:

- `standings` sorts at most eight karts: three times on a tick with bots,
  once without, and once more on a tick a kart finishes. The cost is three
  small allocations a call.
- The calls do not want the same standings. Bot steering and item
  targeting read the positions before the karts move; the `Finished` event
  reads them after. One stored value per tick would have to be two.
- `cross_checkpoint` writes five `Kart` fields in one function of thirty
  lines; the rule ADR 0022 settled lives there whole.
- The ordering tests in `laps.rs` drive the oval: two to three seconds of
  sim time for an overtake and a dead heat, one oval lap for laps against
  checkpoints. None needs a Brands Patch lap.
- The module would move the sort and the crossing rule rather than delete
  them, and testing it apart from `step` would need a record of its own
  (ADR 0027).

## Decision

- `standings` stays a function that sorts when called. `step` does not
  cache standings.
- The crossing rule stays in `cross_checkpoint`, tested through `step`.

## Consequences

- Each caller gets standings as of the moment it asks, with no stale store
  to reason about.
- If the kart count grows well past eight, or a profile shows the sort,
  revisit with the before-and-after split in hand.
