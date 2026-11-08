# ADR 0101: A ramp is a slope across part of the road

## Status

Accepted (2026-11-08).

## Context

Bell Deer's jump at Spoon's exit spanned the whole road: an elevation feature
along the centreline lifted the asphalt, the sand and both walls together, so
every kart jumped and the exit held no choice. Card B5 of
`docs/mockups/course-excitement.html` asks for a ramp on one side with a flat
line beside it, and a boost on its approach: air on one side, the ground and
the tow on the other. A layout's elevation can only say how high the road is
at a spline progress, so a ramp bounded across the road needed a shape of its
own.

## Decision

- **`Ramp(s_start, s_end, height, right, left)`** is a layout field beside
  `elevation`, in metres left of the centreline, and stays on the asphalt. The
  old centreline `Ramp` feature goes; no other layout used one. The centreline,
  the walls, the sand and every placement keep the road's height, so a
  projectile riding the centreline's height passes through the slope's drawing.
- **The sim's ramp is the slope alone, in the `Asphalt` mesh.** It rises out of
  the road's own vertices at the foot. A slope in a mesh of its own threw karts
  off the edge of its foot at up to 17 m/s, since the sim smooths seams only
  inside a mesh. Faces for the sides and lip stopped a kart on the flat half
  dead, and road kept under the slope took the launch's vertical speed away
  (2.3 m/s to none), so it has neither. Measured through `step`.
- **Its sides and lip are scenery**, with kerb stripes down the slope's edges
  and sides, so the driver reads the ramp's half in time to choose. A kart
  cutting in from the side rides up the slope, or under its high end on the
  grass 5 cm below the road, and is never held. The slope's underside is that
  much open.
- **Bell Deer's ramp** takes the left half, the inside of `spoon-2`'s exit,
  with a boost pad on that half leading into its foot; the exit's natural line
  runs out to the flat right half. It moves from 14 m to 36 m along
  `back-straight`, because the first checkpoint past the old lip stood 15 m
  beyond it, nearer than any flight lands, and past that checkpoint a kart lands
  well short of the next.
- **It is 0.5 m high and 8 m long, down from 1.5 m and 12 m.** A pad's boost
  runs by the clock for 1 s, through the air, and the speed falls to the cap the
  tick it ends. From the higher ramp the boost ran out in flight; from this one
  a kart off the pad lands about 0.4 s and 17 m later with 0.2 s of boost left.
- **Bots keep their line**, 2.5 m right of the centreline, which is the flat
  half. Traffic puts some on the ramp's half, and every bot finishes either way
  (0 of 960 bots unfinished over 40 seeds at each of the three levels; the
  200-race stuck sweep stays clean). A bot choosing the ramp on purpose is card
  X5, not this decision.
- The trick boost for a hop on the lip, the other half of B5, waits, so one
  change is judged at a time.
- Bell Deer gains a third boost pad, on top of the count ADR 0097 left it with.

## Consequences

- A ramp on a corner or across the centreline needs no new pipeline: the
  bounds say where, and `ramp_in_the_asphalt` rebuilds the ribbon there.
- The jump is lower for everyone, which costs some spectacle against the old
  ramp; the airtime is bound to the pad's 1 s until a rule keeps a boost in the
  air.
- Bell Deer's replays are re-recorded; each run's laps, coins and stops came out
  as they did on the old ramp.
