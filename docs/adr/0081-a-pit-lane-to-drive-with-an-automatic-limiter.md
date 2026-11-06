# ADR 0081: A pit lane to drive, with an automatic limiter

## Status

Accepted (2026-11-03). Replaces the glossary's "no pit lane to drive",
which was only ever a description of the prop: ADR 0050 does not decide it.
Extends ADR 0079, whose checkpoints must also reach across a lane. The
specification and its evidence are in
`docs/mockups/tyre-wear-and-pit-lane.html`. ADR 0095 supersedes every
level's pit call with a call by the cliff.

## Context

Worn tyres (ADR 0080) need a way back, and the point of the feature is F1's:
a driver gives up time now, at a limited speed, for pace later. An instant
repair or a pickup would not ask that question.

## Decision

- Bell Deer, Goldstone and Monarch each get a pit lane beside the start
  straight, from an optional `pit_lane` in the track sidecar holding its
  path, entry and exit lines and eight pit boxes. A course without one has
  no pits.
- The oval gets none. A 16 s lap makes a stop cost a third of a lap, and
  tyres last a three-lap race there.
- The entry is a visible fork and steering is the whole control: nothing is
  locked in before the entry line.
- The pit limiter engages by itself at the entry line and lets go at the
  exit line. There is no limiter key and no speeding penalty, because one
  more key would give drivers an easy way to lose a race by accident.
- The limit is 54 km/h (15 m/s). It beats coins, the slipstream, a star and
  every boost, and ends any boost on entry. A limit in proportion to F1's
  would be 27 km/h, which feels broken; 54 keeps the cost of a stop right.
- The lane is safe ground. No item can be fired, hazards cannot land, a
  homing projectile gives up, a star cannot spin anyone out, and tyres wear
  only by rolling.
- Every slot has its own pit box and crew in front of its door in the pit
  building, so karts never queue. Reaching it guides the kart onto its mark,
  inputs are ignored and its body stops colliding, so nobody can shunt it.
  The stop itself is ADR 0082.
- "Box" alone would clash with the item box, so the words are `pit lane` and
  `pit box`. BOX appears only as the radio call on the Instrument panel.
- Lanes are about 140 m (Monarch's about 120 m), so a stop's whole cost is
  about 8 s at a median stop: 4.1 s for the slower lane, 1.0 s on and off
  the mark, 2.8 s stationary.
- The lane's surface is asphalt, or the respawn behind a wall (ADR 0063)
  would take karts out of it. Every checkpoint along it reaches across it,
  or a lap through the pits would not count.
- Bots pit by thresholds in `BotProfile`, varied by a few points from the
  seed so they do not all pit on one lap. Basic at 25% or less, Pro at 30%
  or less or if life would run out, Ace by projecting life to the flag.

## Consequences

- Nothing here is built yet. Fit was checked against layout numbers only.
  Bell Deer and Goldstone keep 8.5 m between the wall's outer face and the
  pit building, enough for a 7 m lane; Bell Deer goes first to prove it.
- Monarch has narrower run-off and no pit building, so it costs most. If it
  proves too hard, it can ship without a lane, as the oval does, by a new
  decision.
- Spline progress is found near the last reading (ADR 0046), so a parallel
  lane should hold, but needs a test.
- Unsafe release is left out. Holding green for traffic depends on other
  karts, which the browser's prediction cannot see in time.
- A kart can park in the fast lane. Contact is harmless at that speed and
  `RACE_END_CAP_S` limits how long it can matter.
- At three laps, stopping does not beat managing in the model (ADR 0083),
  so a stop there rescues a kart that was hit or overdriven.
