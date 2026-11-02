# ADR 0021: Karts bounce off each other

## Status

Accepted (2026-10-23). The last point of its decision, that a snapshot does
not carry the bump and the prediction keeps its own on a rewind, is
superseded by ADR 0048. Extended by ADR 0075: a kart on a star's run passes
through ordinary karts, with no contact pair and no bump, and spins them out.

## Context

Karts are dynamic Rapier bodies with no friction and no restitution, and
the handling model sets each kart's velocity from its own speed every
tick. Rapier's contact solver resolved a touch between two karts as a
dead, equal-mass collision, and the next tick's handling threw that
result away: the kart behind set off at full speed into the kart ahead
again, and the solver carried both along at the average. A kart driven
into the back of another pushed it along the track at half speed for as
long as the throttle was held, nose to tail. A side touch let the solver
yaw both karts onto one heading, and they ran glued side by side. Players
described karts sticking and being pulled along.

Mario Kart publishes no collision rule. Its stats pages say weight sets
how hard a kart bumps another and that relative speed and boosts count;
the open reimplementations of Mario Kart Wii's physics (Kinoko, Hanachan,
the doldecomp project) cover a kart against the track and objects, not
against another kart. What every entry shows on screen is the same: a
touch throws the two karts apart, the one that drove in loses speed, and
neither ever carries the other. With no formula to copy, the rule below
is that behaviour done as plain physics.

## Decision

- A touch between karts is a bump. Each kart of the pair takes a velocity
  away from the other: at least `KART_BUMP_MIN_M_PER_S`, plus
  `KART_BUMP_RESTITUTION` times half the speed the two closed at, judged
  by the velocities the handling sent them off with. The handling adds
  the bump to the kart's own velocity every tick, and the bump decays to
  a third every `KART_BUMP_DECAY_S`, then drops to nothing.
- A kart that drove into the other loses `KART_BUMP_SPEED_LOSS` of the
  closing speed from its own, scaled by how square it hit. A kart hit from
  behind or beside loses nothing of its own speed: it is displaced and
  drives on.
- A bump under way is raised to a new one, never added to, so a touch
  that lasts holds the karts apart without flinging them, and the closer
  at the same pace is a steady nudge.
- Rapier still keeps the two boxes from passing through each other
  within the tick; the bump is what carries from one tick to the next.
- A contact never turns a kart. Every rotation axis of a kart's body is
  locked to the solver, and only the steering's yaw, set each tick,
  rotates it: a side touch leaves both headings as they were, and a wall
  met at an angle holds the kart at its corner instead of swinging it
  round.
- Every kart weighs the same, so the pair bump alike. A weight stat, if
  one comes, scales the two bumps by the weight ratio and changes nothing
  else here.
- The values live in `sim::tuning` under their own heading (ADR 0014)
  and the bump is in the state hash, so the replay fixtures were
  re-recorded.
- Kart contact stays the server's (ADR 0009). The prediction runs the
  same rule against the kinematic stand-ins for the other karts, so a
  touch answers at once; a snapshot does not carry the bump, so on a
  rewind the prediction keeps its own and reads the speed from the
  snapshot's velocity with that bump taken off.

## Consequences

- Ramming is a cost: the kart behind bounces back, slows and must
  re-accelerate; the kart ahead is nudged on and keeps its speed. A
  standing kart cannot be bulldozed, only nudged a little at a time.
- Side by side karts that touch part within a few ticks and keep their
  headings.
- The prediction and the server can disagree for the few ticks a bump
  lasts after a touch the other kart's 100 ms of lag puts at different
  places; the correction is blended as any other. Carrying the bump in
  the snapshot would close that gap and is a protocol change for later.
