# ADR 0057: The homing projectile is a camera quad drone that tilts into its turns

## Status

Accepted (2026-10-30). Supersedes the seagull that ADR 0024 chose as the
homing character and ADR 0037 built as the projectile's mesh. What those
records decide about the slot, its states, its accessible text and the
other items' characters stands.

## Context

The homing projectile has flown as a low-poly seagull since ADR 0024. A
gull reads as a bird that happens to be over the road, not as something
sent after you, and nothing about it tells you which way it is about to
go. `docs/mockups/homing-drone.html` compares drone designs for it and
settles on one: a camera quad, a trackside broadcast FPV drone gone rogue,
with its parts, its slot icon, the tilt rule and a live chase demo.

## Decision

- The homing projectile is drawn as a camera quad, built in code from
  Three primitives in `item-meshes.ts` as the mockup's parts table lists
  them: a carbon body under a red canopy, X arms in the lit carbon out to
  orange motors, a two-blade rotor over a translucent white blur disc on
  each, a camera pod under the nose whose red lens is its eye locked on,
  and an antenna at the rear left with a tally light on its tip. Nose
  toward +z, 0.6 m over where the sim has it, as the gull flew.
- Its rotors spin, diagonal pairs the opposite way; its tally light blinks
  at 1.5 Hz; it bobs 2 cm as it hovers.
- The item slot draws it flat and side on, nose right, rotors blurred, the
  lens red. `GULL_EYE` is renamed `DRONE_EYE`; no colour is added.
- It tilts into each turn, left side down to go left and right side down
  to go right. The turn is how fast its facing yaws, measured frame to
  frame, since the View already turns each drone to face its target; the
  difference is wrapped into (-pi, pi] so crossing the yaw seam reads as
  the small turn it is. The tilt is in proportion to the turn rate, capped
  at 30 degrees, which it reaches at 2 rad/s, and eases toward that with a
  0.12 s time constant. The whole drone rolls about its nose after it
  faces its target. A newly fired drone starts level.
- The cap, the full-tilt rate and the ease are View constants in
  `homing-drones.ts`, not in `tuning.rs`: they change nothing the sim
  decides.
- The snapshot lists projectiles in firing order with no id, so a drone is
  matched from frame to frame to the nearest one last frame, closest pairs
  first, within 10 m. A tilt stays with the drone that earned it when an
  earlier one hits; resetting every drone to level whenever the count drops
  would snap the rest level for a moment.

## Consequences

- Presentation only. The sim, the protocol, the bridge and the glossary
  are unchanged; the item is still the homing projectile, homing, hitting
  and timed as before.
- The drones' pool is the first in the View to keep state between frames:
  each drone's last facing and tilt. The cones keep the stateless pool.
- Two drones that pass within a few metres of each other in one frame may
  swap tilts. Both are turning onto the same line then, so the swap does
  not show; a projectile id in the protocol would remove it.
- The cap, the full-tilt rate and the ease are starting values. If the
  tilt reads too little or too much at race speed they are retuned, and
  this record and the style guide follow.
