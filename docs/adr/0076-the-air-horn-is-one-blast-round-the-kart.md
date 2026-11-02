# ADR 0076: The Air Horn is one blast round the kart

## Status

Accepted (2026-11-02). Extends ADR 0025's own-kart audio rule a second
time, by the distance rule ADR 0075 set, and gives the hazard a spline
progress the snapshot does not carry. The design, with the full tables and the
sound's synthesis, is `docs/mockups/star-and-air-horn.html`; the sound was
picked in `docs/mockups/star-and-horn-sounds.html`.

## Context

Every item was fired as soon as it was useful, and nothing let a kart
answer an item coming at it. A homing projectile could only be met with a
trailed hazard, and a kart got no warning that one was coming. Issue 264
adds the Air Horn: one blast, 7 m round the kart, that spins out the karts
in reach and uses up the hazards and homing projectiles in reach. It is the
one item worth saving and timing, and its best moment is destroying a drone
just before it hits.

Three things the existing rules would have got wrong. Distance alone
reaches too far: Bell Deer's crossover puts one road 5 m over the other,
and Goldstone's and Monarch's closest sections are 17.8 m apart centre to
centre, so a 7 m sphere would blast through the bridge and across the
barrier. A hazard had no spline progress, so there was nothing to judge a
hazard on the road by. And a projectile's hit is 0.39 s from the blast's
edge at the speed it closes on a kart at top speed, which cannot be met
blind: neither the Instrument panel nor the audio marked an incoming drone.

## Decision

- `Item::Horn`, code 5, with its column in the roll weights: places 1-2
  roll 15 percent, 3-5 15 and 6-8 five, taken from the hazard's and the
  boost's shares. The horn mostly goes to the front and the middle, which
  have the most to protect and the most karts nearby.
- The blast happens at the fire, before the tick's handling. Fires run in
  slot order, as they did, so of two horns on one tick the lower slot goes
  first and the kart it spins out loses its press: a spun-out kart's press
  is dead. The fire is reported before what it does.
- In reach means within `HORN_RADIUS_M` (7 m) of the firing kart's centre,
  seen from above, with a kart judged by its collision box as every hit
  is, and within `HORN_PROGRESS_WINDOW_M` (15 m) of spline progress the
  shorter way round the lap, so the window wraps at the start line. The
  window keeps distance from reaching through a crossover or across a
  barrier; a `const` assert holds the radius under it, so a kart in reach
  by distance on the same road is never dropped by the window.
- What the blast meets: another kart spins out, loses the spin-out's coins
  and is reported `SpunOut { by: Some(firer) }`; a kart trailing a hazard
  loses the hazard in its place and drives on, the rule a projectile
  follows; a kart already spun out and a star kart are left alone, the
  star kart's trailed hazard with it. Every hazard lying on the road in
  reach is used up, whoever dropped it, the firer's own included; the
  hazard the firer trails stays. Every projectile in reach is used up,
  whoever it chases, so a blast can save a neighbour. The runaway wheel,
  the item boxes and the coins are not touched. With nothing in reach the
  blast still fires and the item is spent.
- `PlacedHazard` records its owner's spline progress at the fire, keeps it
  up while the hazard trails, and so holds the owner's at the drop. It is
  in the state hash and stays out of the snapshot: the View never reads it.
- Bots fire a horn as soon as its blast would spin a kart out, by the same
  test the blast uses on the readouts, and otherwise once it has been held
  for `BOT_HORN_HOLD_S` (10 s). That replaces the first-straight rule and
  the use delay for this item. A bot reads karts and not projectiles, so it
  fires at no drone chasing it.
- Protocol: `Item` gains `Horn` and the version goes up. No new snapshot
  field: everything runs off the existing `ItemFired`, which every client
  now acts on for every kart, not only its own.
- Presentation: the run reports each blast for 0.5 s from the server's
  event, numbered, with the position the kart was drawn at as the event
  arrived, the interpolated other karts' or the predicted own kart's. The
  View grows a flat orange ring on the ground to the reach in 0.25 s and
  fades it out, on every screen, so the reach can be learned by eye. The
  slot shows the grandstand air horn, scowling, still when held as ADR 0024
  asks, and flashes on firing like any other item. There is no horn mesh:
  the blast's ring is all the road shows.
- The drone warning: the Instrument panel shows the drone's red eye beside
  the slot, blinking, while a homing projectile chasing the own kart is
  within `DRONE_WARNING_M` (40 m), about 2.2 s out; steady under reduced
  motion. Snapshots already name each projectile's target, so this is
  client code only, and it helps every kart whatever it holds.
- Audio, extending ADR 0025 by ADR 0075's distance rule: `air-horn` plays
  in place of `item-fire` for the own kart, from the slot emptying; every
  other kart's blast plays once, the frame the run first reports it, full
  within 10 m and a straight line to silent at 60 m through the 1200 Hz
  lowpass, with no panning. `drone-beep` repeats while the warning holds,
  every 0.35 s at 40 m closing to every 0.1 s at the kart, its pitch
  rising by half, on the audio's own clock from the distance the reading
  carries: the one sound that is not an edge. Both sit under the SFX
  toggle.
- The replay fixtures are re-recorded, since the hash gains the hazard's
  progress.

## Consequences

- A blast at top speed costs each kart it reaches about 68 m and three
  coins, and the firer nothing: it is the one item with no cost to hold.
- The hazard's state grows by a float the snapshot never carries, so the
  wire is unchanged but the state hash is not.
- A kart with a horn can wait out a drone: the warning gives about 2.2 s
  and the blast's edge 0.39 s, so the save is a timed press and not a
  guess. Bots do not defend yet; passing the projectiles to `bot::drive`
  waits for feel testing.
- Other karts' horns are the second sound heard from another kart, after
  the star music; their engines, fire and end stay silent. Stereo panning
  stays out of scope.
- A hit's sound, a horn mesh on the road and any new snapshot field are
  out of scope.
