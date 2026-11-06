# ADR 0093: Step reads the kart's place where it needs it

## Status

Accepted (2026-11-06). Extends ADR 0029 to the pit lane, the limiter and
the respawn.

## Context

The architecture review of 2026-11-04 (issue 299, candidate 6) proposed a
module that answers where a kart is on the course: in the pit lane, at its
pit box, off course, its spline progress and whether it is going the wrong
way, with `step` reading the answer once a tick. It noted the lane flag
computed twice a tick.

Against the code after issue 297 moved the items' hits out of `step`, the
review found:

- The lane is read at three moments in a tick, and each reads a different
  position. Before the karts move, it decides who may fire or hold an item
  (ADR 0081). After a kart moves, it holds that kart to the pit limit
  before the off-course check, which can respawn the kart onto the lane.
  After every kart has moved and respawned, it decides who items and stars
  cannot touch. One stored answer would have to be three, as ADR 0029
  found two for standings.
- Each reading is one call to `Track::in_pit_lane`, which owns the rule.
  The repeats are calls, not copies of the rule.
- The pit-box catch, the off-course timer, the lane respawn and the wrong
  way timer each read the previous tick's `pit_lane_m` or spline progress
  together with this tick's, so they are crossings between two readings,
  not one place.
- `cross_checkpoint`, `hold_in_pit_box`, `respawn_on_lane` and
  `respawn_at` are small functions that each write the fields they own.
  The module would move them rather than delete them.
- The behaviours are tested through `step` (`laps.rs`, `crossing.rs`,
  `pit_lane.rs`, `stop.rs`); the module would add no test the seam lacks.

## Decision

- `step` reads the lane, the pit box, the off-course state, the spline
  progress and the wrong way where it acts on them, through `Track` and the
  helpers it has. No course-place module, and no place stored for the
  tick.

## Consequences

- A rule that needs where a kart is calls `Track` at the moment in the
  tick it needs, and says in a comment which position that is.
- Revisit if two readings of the same moment in the tick start to disagree,
  or a new rule needs the place at a moment `step` does not yet read it.
