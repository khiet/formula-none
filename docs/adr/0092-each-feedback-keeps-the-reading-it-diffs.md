# ADR 0092: Each Feedback keeps the reading it diffs

## Status

Accepted (2026-11-06). Keeps ADR 0028 and ADR 0030 as they are.

## Context

The architecture review of 2026-11-04 (issue 299, candidate 4) counted ten
modules that keep a previous reading and diff it, four of them to find the
end of a stop. It proposed that the race readout carry the own kart's
edges once a frame, so each Feedback reads them instead of keeping a
previous reading.

Against the code after issue 296 carried the kart readout on the wire as
it is, the review found:

- Most of the stores are not about the own kart. The sparks' last tier,
  the exhaust flame's previous boost, the View's coins before and label
  shine, the head sway's last readout and the pit box's `wasStopping` are
  kept for every kart in the race. Only the audio cues and the chase
  camera's shake diff the own kart alone.
- Not every store is the same edge at the same cadence. `server-race.ts`
  finds a stop's end between snapshots, not frames. The Instrument panel's
  `shown*` values are what it last wrote to the DOM, held so it writes
  only on a change. `createBoxCall` latches a threshold once a race; it
  keeps no previous reading.
- The four that see a stop end want the ended stop, not a flag: the pit
  box lights green and holds the stationary time, the panel shows the
  final time and tier, the audio picks its beat from the stop clock, and
  the server keeps the shortest stop seen.
- The audio's `before` is not the last reading. `heardOnward` holds the
  stop clock at the furthest it reached, so a reconciliation that steps
  the clock back does not sound a beat twice. A shared edge set would
  have to pick one meaning of "before", and the audio needs its own.
- The stop-ended diffs cost about four lines each. A shared edge set would
  need its own previous readings of every kart, its type, and the fields
  that carry each ended value, which is more than it deletes.

## Decision

- Each Feedback, the audio cues and the chase camera included, keeps the
  previous reading it diffs. The race readout carries readings, not edges.

## Consequences

- An edge two Feedback both need is diffed twice, a few lines each, in the
  module whose cadence and notion of "before" it follows.
- `soundReading` keeps copying the fields the cues diff, as ADR 0028 has it.
- Revisit if several Feedback come to need the same own-kart edge at frame
  cadence with the same reconciliation rule, so one store would serve all
  of them unchanged.
