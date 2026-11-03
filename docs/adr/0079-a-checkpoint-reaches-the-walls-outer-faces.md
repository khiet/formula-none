# ADR 0079: A checkpoint reaches the walls' outer faces

## Status

Accepted (2026-11-03). Supersedes the clause of ADR 0022 that a checkpoint
spans the run-off from wall to wall; the rest of ADR 0022 stands.

## Context

A checkpoint reached the inner face of each wall, on the reasoning that a
kart cannot be further out and still be on the course. It can: ADR 0063
records karts clearing a wall, and leaves how they do it alone.

In 200-seed sweeps of eight bots on Monarch, two karts ended the race a lap
short (Basic seed 60, Ace seed 96). Each passed its next checkpoint in the
air, 2.5 to 3 m over the checkpoint and about 8.2 m from the centreline, above
the 1 m wall, and landed back on the course. The trace of Ace seed 96 shows
how: at 39 m/s on a boost it met a wall of the 2 m run-off and rode up it.
Nothing respawned either kart, since neither came down behind the wall, and
the checkpoint went uncredited: the kart drove a whole extra lap to cross it
again and the end cap fired with it a lap short. The two karts missed
different checkpoints, 6 and 7, so the cause is the rule and not one gate's
place on the lap.

## Decision

- Every checkpoint on every course reaches the outer face of each wall: the
  run-off's half width plus the wall's thickness.
- How a kart gets over a wall is still not changed here.
- A kart beyond the outer face stays the respawn's to judge (ADR 0063).

## Consequences

- In 600 seeds at each bot level on Monarch, run once by hand, no kart is left
  unfinished; before, one was at Basic and two at Ace. The sweep the repo keeps
  runs 200 seeds at each level.
- A kart that flies further out than the outer face and lands back inside can
  still miss a checkpoint and lose the lap; none did in those seeds.
- A kart cannot gain from the extra half metre: a checkpoint still counts
  only as the next in order, and the walls still close every cut.
- The sidecars change in `half_width` only; the glTFs do not. The oval, Bell
  Deer and Goldstone have wider run-off that slows a kart before a wall, and
  600 seeds at each level of Bell Deer and Goldstone left the same karts
  unfinished at the wider reach.
