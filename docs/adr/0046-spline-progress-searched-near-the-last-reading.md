# ADR 0046: Spline progress is searched near the last reading

## Status

Accepted (2026-10-29). Supersedes nothing.

## Context

Spline progress was the distance along the centreline to the nearest
centreline point over the whole lap, seen from above. Where a course crosses
over itself that point can lie on the other leg, so a kart beside its own
leg at the crossing reads hundreds of metres on or back. The Suzuka rework
of Brands Patch (`docs/mockups/suzuka-course.html`) has such a crossing: the
back straight runs over the underpass on a bridge 5 m high. A projectile
rides at the centreline's height where it is, so on the same mistake it
drops off the bridge to the road beneath.

## Decision

- A reading is searched only over the centreline within
  `PROGRESS_WINDOW_M` either side of the last reading, wrapping past the
  start line. `Track::progress_near` does this; `Track::progress_at` keeps
  the whole-lap search.
- The window is wider than the furthest a kart or projectile travels in one
  tick, and than a kerb a bot steers for lies ahead of it. It is narrower
  than the lap distance between two legs of a course that cross.
- A kart reads near its last reading every tick. It reads the whole lap
  where it has no last reading: at spawn, and after a respawn.
- A projectile carries its own last reading, set at its fire from its
  owner's, for the height it rides at and the point it steers for.
- A bot reads its kart's spline progress where it looked up the
  centreline beside the kart, and looks up a kerb's progress near it.
- A checkpoint's progress is found over the whole lap when the track is
  built, since it has no last reading. On the reworked course every
  checkpoint sits at least 24 m from the bridge.
- Snapshots already carry each kart's spline progress, so the client's
  prediction searches from the same reading as the server. The protocol
  does not change.

## Consequences

- On a course that crosses itself a kart and a projectile keep their own
  leg through the crossing. On today's tracks, which do not cross, the
  readings are as before, and every replay fixture keeps its hashes.
- The rule assumes a single centreline. A pit lane or a branch would need
  its own progress, and is not covered.
- A kart moved a long way in one go, as by a respawn, must read the whole
  lap; a new way to move a kart must do the same.
- A kart driven far off the course, so the leg nearest it is beyond the
  window, reads the nearest point within the window instead. On today's
  tracks walls keep a kart close enough that this does not arise.
