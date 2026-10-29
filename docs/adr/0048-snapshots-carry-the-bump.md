# ADR 0048: Snapshots carry each kart's bump

## Status

Accepted (2026-10-29). Supersedes the last point of ADR 0021's decision,
that a snapshot does not carry the bump and the prediction keeps its own
on a rewind.

## Context

ADR 0021 made a touch between karts a bump: a velocity away from the
other kart that the handling adds to the kart's own every tick and that
decays over `KART_BUMP_DECAY_S`. A snapshot carried the kart's velocity
only, so on a rewind the prediction kept its own bump and read the speed
as the snapshot's velocity with that bump taken off. Where the server's
touch and the prediction's differ, as they do when the other kart's
100 ms of interpolation lag puts the touch at a different place, the
prediction read the server's bump as driving speed: a kart hit from
behind while idling was predicted driving off at the bump's speed, and
the speed then ran down by the handling's rule while the server's bump
decayed by its own, so every snapshot corrected the kart until both had
died away. ADR 0021 left carrying the bump in the snapshot as a protocol
change for later; issue 64 is that change.

## Decision

- The kart readout and the kart snapshot carry the bump, as an optional
  vector that is present only while the kart carries one, since a spent
  bump drops to zero (ADR 0021) and most karts carry none most of the
  time. The protocol version moves to 11.
- On a rewind the prediction takes the snapshot's bump in place of its
  own and reads the speed as the snapshot's velocity with that bump taken
  off, the way the handling built the velocity.
- A snapshot agrees with the prediction only when its bump matches the
  record's to the reconciliation tolerance, as its velocity must, so a
  snapshot that differs in the bump alone is taken rather than passed
  over.
- Kart contact stays the server's (ADR 0009). The prediction still runs
  the bump rule against the kinematic stand-ins for the other karts, so a
  touch answers at once.

## Consequences

- A touch the server places elsewhere than the prediction corrects the
  own kart once, blended over the tuned ticks like any other correction,
  and the snapshots after it agree while the bump runs down; there is no
  second correction once it has decayed.
- A kart hit while idling is predicted as the server has it: carried by
  the bump with a speed of nothing, not driving.
- The busy sample snapshot grows by the one bumped kart's vector and the
  other karts' absent one (ADR 0010's Outcome records the size). The
  budget holds while few karts carry a bump; a pile-up of all eight passes
  the size test's cap for the ticks their bumps last.
