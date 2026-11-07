# ADR 0096: The BOX call comes as the kart nears the pit lane

## Status

Accepted (2026-11-07). Supersedes ADR 0080's BOX plate and chime for 2 s
as life first falls to 30%. ADR 0080's gauge, its bands and the smoke
stand, as does the plate's spot beside the gauge, in white on
`POSTER_RED`.

## Context

The call came once, at the moment the own kart's tyre life first fell to
the cliff, wherever the kart was on the lap. That was usually far from the
pit lane, so by the time the kart reached the fork the call was long gone,
and it never came again on the laps a driver stayed out on worn tyres. The
gauge turns red at the same moment, so the call told the driver nothing
they could act on.

## Decision

- The Instrument panel reads `BOX BOX`, with the box chime, while all of
  these hold for the own kart:
  - tyre life is at or below the cliff (30%);
  - it is within 120 m of the pit lane's fork and more than 15 m short of
    it, measured along the spline and wrapping at the line, as the bots
    measure it: about 4 s at top speed to hear the call and move across,
    and nothing where there is no longer room to get across;
  - it is not in the pit lane;
  - the lap on which it reaches the fork is not the final lap.
- The call opens at most once for each pass of the fork, so a
  reconciliation that moves progress or life back over an edge neither
  reopens it nor sounds the chime again. If life falls to the cliff inside
  the window, the call shows for the rest of it.
- Both distances are presentation constants beside the call, not the
  sim's tuning. The bridge hands the web the fork's spline progress and
  the spline length; the sim already works the fork out for the bots.
- Unchanged: the own kart only, and no call with Tyres Off or on a course
  without a pit lane.

## Consequences

- The call comes every lap a worn kart stays out, and the chime with it.
- The call ends as the kart enters the pit lane, so the BOX plate never
  shares its spot with the STOP plate.
- A kart that falls to the cliff inside the last 15 m before the fork, or
  after it, hears nothing until the next lap's fork.
