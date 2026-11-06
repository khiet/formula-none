# ADR 0089: Goldstone's pit lane moves to the real side

## Status

Accepted (2026-11-06). Supersedes ADR 0088's keeping of Goldstone's lane left
of its start straight, ADR 0085's placing of it there, and ADR 0060's pits
outside the Hamilton Straight at 120 m. Extends ADR 0081's bot pit call with
how fast a bot called in drives at the lane.

## Context

Silverstone's pit lane runs right of the Hamilton Straight (OpenStreetMap way
227902927), inside Club and Abbey. ADR 0088 moved Monarch's lane to the right
and left Goldstone's on the left, because a lane inside Club could not be
driven by a bot.

Goldstone's lane on the right fits: its fork at Club's start, its lines 52 m
short of the start line and 92 m past it, its merge 115 m past it. It measures
139 m between the lines, near ADR 0081's 140 m, and its outer wall folds
nowhere: Club's 20 m radius just reaches the wall's back, 20 m out.
But its path sweeps out while Club turns, and turns at about 6.6 m. A bot called
in slowed only to its 20 m/s approach, where full lock turns about 13 m, so it
ran wide and missed the fork on every call.

A bot already slows for a corner by the centreline's curvature ahead. The lane's
own path was never read that way. Reading it changes nothing in Bell Deer's or
Monarch's replays.

A layout change was weighed: a Club of 30 to 35 m radius gives the sweep a 17 to
22 m turn, but the lap grows by 100 to 160 m.

## Decision

- Goldstone's lane runs right of the Hamilton Straight. It forks at Club's
  start, sweeps out round Club's inside, runs past the line round Abbey's
  inside, and merges on the run to Village. Its entry line is 52 m short of the
  line and its exit line 92 m past it.
- A bot called in also slows for the sharpest turn of the lane's path within its
  braking horizon, at the speed it takes a corner of that curvature, until it
  reaches the entry line. It reads the path from where it stands on the lane's
  road, or from as far short of the road's start as it is short of the fork.
- Goldstone's pit building moves right of the straight, between Club and Abbey,
  70 m long with its middle 6 m short of the line and its doors centred on it.
  There it keeps the 8 m rule with 8.5 m of grass to the nearest wall; a
  building much longer would not, 74 m keeping only 7.9 m wherever it stands. It stays white. The grid's stands move left, across from it.

## Consequences

- Every lane now runs on the side its real circuit's does.
- A lone bot pits on Goldstone as often as it did with the lane on the left, and
  finishes five laps about 1 s sooner, over 40 seeds at each level. Inside Club
  it slows below its 20 m/s approach, to the speed of the sweep's turn.
- Goldstone's pits are now shorter than Bell Deer's 90 m, so "longer" no longer
  sets them apart; the white walls still do.
- Bell Deer's and Monarch's replays are unchanged. Goldstone's are re-recorded,
  and its pit replay races seed 3 in place of seed 7, whose first stop now
  comes after the fixture's 140 s.
