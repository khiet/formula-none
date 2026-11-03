# ADR 0055: Tyres collected on the track raise top speed; a spin-out costs three

## Status

Accepted (2026-10-30). Supersedes the consequence of ADR 0031 that the sim
does not wear tyres and that tyre wear, a sim field with wear per corner
and an effect on grip, would replace the gauge's placeholder: the gauge
reads the count of tyres a kart carries instead. The rest of ADR 0031
stands. Its choice of object, the tyre, and the gauge's reading of the
count are superseded by ADR 0056; its rule stands, but for the clause that
a kart takes one only while it carries fewer than ten, superseded by ADR
0067. ADR 0080 brings wear back, so the sim wears tyres after all.

## Context

The Instrument panel's tyre gauge has held at 100% since ADR 0031, waiting
on a tyre wear the sim never had. Issue 190 settled a rule that gives the
gauge something to read and the straights something to race for: tyres lie
on the asphalt, a kart collects them up to a set, each one raises its top
speed a little, and a spin-out costs some. Issue 191 carries the rules,
the wire and the layouts; issue 192 draws them.

## Decision

- A tyre is a placement in the track sidecar, beside the item boxes and
  boost pads, laid by the Blender layouts as lines of tyres along the
  direction of travel on straights the box rows do not use, some off the
  centreline. About ten to twelve on the oval and fifteen on Suzuqa Patch,
  ten of them on the centreline, so one clean lap collects a full set. No
  tyre lies on a box, a boost pad or a grid slot.
- A kart whose centre is within the tyre's tuned half-length and half-width,
  airborne or not, takes it while it carries fewer than the tuned set of
  ten; the tyre is gone for the tuned 3 s, then back. A spun-out kart takes
  none. Every kart starts the race with none.
- Each tyre carried multiplies forward top speed by one more tuned 0.01:
  ten make 30 m/s 33. The off-track cap and a boost multiply on top of it.
  Acceleration, reverse and handling are untouched.
- Every spin-out, from a hazard, a homing projectile or the runaway wheel,
  costs the tuned three tyres, or all the kart carries when fewer. Lost
  tyres vanish. A bump against a kart or a tyre wall costs nothing.
- The constants live in `crates/sim/src/tuning.rs` (ADR 0014) and the
  client reads them through `Tuning`. Compile-time invariants hold a full
  set's boosted speed under the homing projectile's, and the
  tick-of-flight and tick-of-travel bounds at a full set's speed.
- The server holds the count. A pickup is an event naming the tyre, as a
  box pickup names its box, and a loss is an event with its count, after
  the spin-out's Hit. The kart readout and every snapshot carry the count,
  and every snapshot carries which tyres are there to pick up. A client's
  prediction never takes a tyre: it adopts the snapshot's count, and a
  pickup the server makes is a correction the size of the speed it adds
  over the round trip, under what a player can see (ADR 0009).
- Bots take what they drive over and lose three on a spin-out, as humans
  do. They do not steer for tyres; the rubber band absorbs the difference.

## Consequences

- The protocol version is 13 and the replay fixtures are re-recorded. The
  tail of `brands-patch-chicane.replay.json` was rewritten so its kart,
  two percent faster on the tyres it keeps, still crosses the chicane's
  kerb; the fixture set now has to take a tyre and lose tyres.
- A tyre changes a race's timing from where it is taken, so a seeded test
  of bots' lines on a track with tyres is a different race. The sim tests
  that hold bots to their lines, and those that measure a speed against
  the bare top speed, run on the track without its tyres.
- The gauge, the tyre mesh, the gear bands stretched to the raised top
  speed and the pickup cue are the client's, in issue 192, with their
  style guide entries.
- Scattering lost tyres, bots steering for tyres and tyres on the course
  map are out of scope.
