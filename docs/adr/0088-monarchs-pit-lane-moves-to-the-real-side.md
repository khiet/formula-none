# ADR 0088: Monarch's pit lane moves to the real side

## Status

Accepted (2026-11-06). Supersedes ADR 0085's placing of Monarch's lane left of
its start straight; the rest of ADR 0085 stands, Goldstone's placing with it.

## Context

All three lane courses run clockwise, so the right of the start straight is
the inside. Monaco's pit lane runs right of the course from end to end
(OpenStreetMap ways 850261588 and 1388331347, "Voie des stands" and "Sortie
des stands"), and Suzuka's runs right of its main straight (way 120917578,
"Pit Lane", 15 to 77 m right of way 183391665, "メインストレート", which is
one-way in the race's direction). Silverstone's runs right too (way
227902927), inside Club and Abbey. Bell Deer's lane was already on the right;
Goldstone's and Monarch's were on the left.

A lane is the run-off swept out until its outer wall's back is 8 m past the
run-off's wall: 16 m from the centreline on Monarch and 20 m on Goldstone
(ADR 0081). Inside a corner of smaller radius than that, the wall
passes the corner's centre and folds over itself. Monaco's lane rejoins past
Sainte Devote, a 13 m right-hander, so a merge there folded the wall and turned
the lane's path at a 2.6 m radius.

Goldstone's lane on the right runs inside Club, a 20 m right-hander. The lane
fit and kept ADR 0081's 140 m, but its path turned at about 6.5 m inside the
corner. A Pro bot called in there ran wide at its 20 m/s approach and missed
the fork on every call. A fork at Club's exit failed too: the bot leaves Club
on the outside and cannot cross to the lane in time, and that lane came to only
about 133 m.

## Decision

- Monarch's lane runs right of its start straight. It forks on the Noghes flick
  75 m short of the line, after Anthony Noghes and not before it as Monaco's
  does, since no lane fits inside that 14 m corner. Its entry line is 50 m
  short of the line, its exit line 70 m past it, and it merges at Sainte
  Devote's start, 93 m past the line, short of the corner and not past it as
  Monaco's does. That keeps ADR 0081's 120 m between the lines and its stop
  cost.
- Monarch's pit building moves right of the straight, still 90 m long, its
  middle 34 m past the line, its front 1 m behind the lane's wall. There it
  keeps the 8 m rule from the second pool chicane's wall behind it. The grid's
  stands move to the left, across from it.
- The pit building's doors start where the layout's `PitLane.door_first`
  says, in metres from the building's middle; the pit boxes line up on the
  same number. It defaults to 2.75 m, which keeps Bell Deer's and Goldstone's
  doors running forward from the line. Monarch's centres its eight doors on
  the building, so all eight boxes stand short of the exit line.
- Goldstone's lane stays left, outside Club and Abbey, until a layout change
  or a bot change lets a lane inside Club be driven. That is a follow-up, not
  part of this decision.
- The track tests hold each lane to its side, and hold every lane's outer wall
  as far from the road as it was built, which a fold inside a corner breaks.

## Consequences

- Monarch's grid faces the pits across the straight on the side Monaco's does,
  and its boost pad on the pit bend, left of the centreline, is away from the
  fork.
- Monarch's replays with bots are re-recorded, since the lane, the stands and
  the building moved.
- Goldstone still has its lane on the side Silverstone's does not.
