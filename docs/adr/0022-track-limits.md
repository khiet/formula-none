# ADR 0022: Track limits are walls along both edges of the run-off

## Status

Accepted (2026-10-24).

## Context

Off-road driving is allowed and costs only the off-road speed cap (story
19: sand and grass slow the kart). Brands Patch's two hairpin infields were
open grass, so a cut across either that the checkpoints refused to credit
was still drivable, and a kart that took it by accident was left with a
lap that would never count. Two rules were added to deal with that: a
checkpoint at hairpin one's apex, narrowed on the inside so a cut could
not reach it, and a respawn for a kart that drove on past a missed
checkpoint. Each was a special case layered on the checkpoint order, and
the second moved a kart against its will for a mistake it could have
driven back from.

Racing circuits settle this physically: a wall where a cut would gain. The
game already has walls that hold a kart (story 20) and a sim that collides
with any tagged mesh, so the course can be walled without a new mechanism.

The first cut of this decision walled the hairpin infields only and hid
one shortcut through a 3 m gap, with the export checking every other pair
of checkpoints for a cut a wall did not close. Played, the call was that
no shortcut should be possible anywhere, and a course walled all the way
round needs no such check.

## Decision

- A wall stands along both edges of the run-off the whole way round every
  track: on the sand's outer edge, following the centreline and its
  elevation over the hill and the ramp. The export builds the two walls
  from the centreline; a layout declares none.
- The checkpoint order is the only judge of a lap. A lap counts when every
  checkpoint is crossed in order and the start line after them; nothing
  else credits or refuses one. The checkpoints are evenly spaced round the
  lap and span the run-off from wall to wall, so a kart between the walls
  crosses every one in order and none can be skipped.
- Off-road driving between the walls is allowed and costs only the speed
  cap. A kart can leave the asphalt for the sand but never the course.
- A missed checkpoint is not respawned. The kart drives back through it;
  the wrong-way warning is the only hint. The respawn covers the edge of
  the world only (story 21), as it did before the missed-checkpoint rule.
- There is no shortcut.

## Consequences

- No corner can be cut, so story 19's "cutting costs more than it gains"
  holds everywhere by construction rather than by checking each pair of
  checkpoints.
- Grass and the edge of the world lie beyond the walls, so on the shipped
  tracks neither is reached by driving. The off-road cap on grass and the
  out-of-world respawn remain the sim's rules for any kart that gets there;
  the tests that cover them take the oval's walls away first.
- Both tracks have twelve evenly spaced checkpoints. The checkpoint index
  is in the state hash, so the replay fixtures were re-recorded, and the
  hop-drift fixture was rewritten to drift between the walls, through the
  oval's first turn.
- The apex checkpoint and the missed-checkpoint respawn are gone, with
  their tests, the per-kart timer in the state and the spawn change that
  came with the respawn. A kart that misses a checkpoint by accident
  recovers by driving back through it.
- A wall met at a slant slides the kart along it; met square, it stops the
  kart dead. The feel of that collision is unchanged and out of scope, as
  are the pipes and the out-of-world respawn itself.
