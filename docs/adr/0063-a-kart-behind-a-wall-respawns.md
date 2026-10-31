# ADR 0063: A kart behind a wall respawns

## Status

Accepted (2026-10-31). Supersedes the clause of ADR 0022 that the respawn
covers the edge of the world only; the rest of ADR 0022 stands.

## Context

ADR 0022 walls every course along both edges of the run-off, so a kart can
leave the asphalt but never the course, and keeps the respawn for the edge
of the world. Grass lies beyond the walls, and the sim's rules for it (the
off-road cap, no respawn) are for a kart that got there anyway.

On Bell Deer and Goldstone none ever does: 6 m of sand slows a kart before
it reaches a wall. Monarch (ADR 0064) has 2 m of run-off. In 40 races of
eight bots, karts that took a Boost from the quay's item row reached La
Rascasse, a 12 m right-hander, at 42 m/s and met the outside wall with
their speed intact; four went over it. Once on the grass beyond, no way back
crossed the wall, nothing respawned them, and each sat out the rest of the
race until the end cap placed it last. A human would be stranded the same
way. A taller wall stopped those four, but with a 3 m wall a kart went
through it instead, so the height was hiding the cause, not settling it.

## Decision

- A kart with a wall between it and its point on the centreline, judged
  level at a kart's height over the line, is off the course. Off the
  course for `RESPAWN_DELAY_S` without a break, it respawns at its last
  checkpoint facing the next, exactly as a kart out of the world does: the
  same timer counts both.
- A kart held against a wall from the run-off is not behind it, and on a
  track with no walls nothing stands between a kart on the grass and the
  line, so the grass rules and their tests, which take the oval's walls
  away, are unchanged.
- How a kart gets over or through a wall is not changed here.

## Consequences

- In the same 40 races on Monarch three karts still clear the wall at La
  Rascasse; each is back on the course 1.5 s later and every bot finishes.
- Bell Deer and Goldstone see no change: no kart reaches their grass. No
  replay fixture had a kart behind a wall, so none was re-recorded.
- The state hash already carried the respawn timer; it is renamed from
  `out_of_world_s` to `off_course_s` for what it now counts.
