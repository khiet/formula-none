# ADR 0022: Track limits are checkpoints and walls, with one hidden shortcut

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

Racing circuits settle this physically: a wall where a cut would gain, and
anywhere a wall is not is fair game. The game already has walls that hold a
kart (story 20) and a sim that collides with any tagged mesh, so the
infields can be walled without a new mechanism.

Walls cannot replace the checkpoints. The hill straight and the jump
straight run parallel 36 m apart with 12 m of open grass between them, and
crossing it skips 250 m of track; only the checkpoint order catches that.
Evenly spaced checkpoints also cap what any legal cut can gain: the
biggest consecutive pair is a hairpin infield, 36 m straight against 66 m
round. A shortcut worth taking needs its skipped arc to hold no
checkpoint, which no layout does by accident.

## Decision

- The checkpoint order is the only judge of a lap. A lap counts when every
  checkpoint is crossed in order and the start line after them; nothing
  else credits or refuses one.
- Walls make a cut worth more than the threshold undrivable. The export
  refuses a layout with two consecutive checkpoints joined by a straight
  line more than 15 m shorter than the centreline between them, unless a
  wall stands across that line or the pair is the layout's declared
  shortcut. Brands Patch walls the inside edge of both hairpin infields,
  hairpin two's from the chicane's south side round the hairpin, so that
  the jump straight is reached only through the declared gap.
- Off-road driving is otherwise allowed and costs only the speed cap.
  Where no wall stands, sand and grass are open.
- A missed checkpoint is not respawned. The kart drives back through it;
  the wrong-way warning is the only hint. The respawn covers the edge of
  the world only (story 21), as it did before the missed-checkpoint rule.
- Each track may declare one shortcut: a 3 m gap in a wall, the kart being
  1.2 m wide, whose skipped arc holds no checkpoint. The export opens the
  gap in the wall covering that arc length, fails if none does, places no
  evenly spaced checkpoint inside the skipped arc, and exempts the pair
  either side of it from the spacing check. Brands Patch's is on the
  approach to hairpin two, leading over 12 m of infield grass to the jump
  straight. It is unsignposted: no scenery, no change to the grass, no
  help text. Bots follow the centreline and never take it.

## Consequences

- Story 19's "cutting costs more than it gains" no longer holds at the
  shortcut, by design. Measured in the sim against a kart following the
  centreline, the shortcut is worth about three quarters of a second a
  lap: the hairpin is rounded at top speed, and the shortcut's sand and
  grass are driven at the off-road cap. A mistimed entry meets the wall
  beside the gap and stops the kart as any wall does.
- Brands Patch has eleven checkpoints: the twelve evenly spaced ones less
  the one on hairpin two's apex, inside the skipped arc. The checkpoint
  index is in the state hash, so the replay fixtures were re-recorded.
- The apex checkpoint and the missed-checkpoint respawn are gone, with
  their tests, the per-kart timer in the state and the spawn change that
  came with the respawn. A kart that misses a checkpoint by accident
  recovers by driving back through it.
- A layout author who wants a cut closed walls it; one who wants a cut
  open declares it the shortcut. There is no third option, and the export
  says which it wants when a pair fails the check.
- Out of scope and unchanged: bots taking the shortcut, a shortcut on the
  oval, paving the shortcut, the out-of-world respawn, the pipes, and the
  feel of a wall collision.
