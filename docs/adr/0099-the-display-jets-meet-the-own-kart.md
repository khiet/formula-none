# ADR 0099: The display jets meet the own kart

## Status

Accepted (2026-11-07). Supersedes ADR 0050's rule that a moving prop moves
from the View's own clock, for the display jets only.

## Context

Goldstone's display jets flew a 600 m pass over the hangars once every 30 s
of the View's own clock (ADR 0050), so a driver often ran the Hangar
Straight with the jets elsewhere, and a lap of about 45 s could miss them
altogether. The course's flypast should be the moment of the lap
(`docs/mockups/course-excitement.html`, card G5): the formation seen
coming as the kart leaves Chapel and over the hangars as it runs down the
straight, every lap.

## Decision

- The jets' pass is cued by the own kart's spline progress, as the
  presentation zones are (ADR 0098): it comes as the kart moves forward
  onto or over the start of `chapel`, never on a lap's wrap over the line
  or a step back. A kart spun back over the cue while the pass it set is
  still to fly or flying does not restart it. Nothing goes on the wire;
  each client's jets meet its own kart, and the sim, the sidecar and the
  replays are untouched.
- The formation is over the middle of its pass 4 s after the cue, the time
  a kart at 25 to 30 m/s takes from Chapel's start to the Hangar Straight's
  middle, so it is within the hangars' 110 m row as the kart gets there.
  The pass keeps its 600 m in 6 s, against the race, so it starts a second
  after the cue, as the kart leaves Chapel, coming at it from Stowe.
  Between passes the formation is hidden, and before the first cue it has
  not flown.
- The cue's progress is a client constant, `GOLDSTONE_CHAPEL_M` in
  `apps/web/src/scenery-motion.ts`, which the track tests hold to the
  layout. A course with no cue never flies its jets.
- An audio swell sounds with each cue: a jet roar rising from afar to its
  peak 4 s on, as the formation is overhead, then dropping in pitch as it
  goes. It is one SFX that `assets/audio/build.py` renders, under the SFX
  toggle, not positioned.
- The smoke ribbons stay as they are: fixed to the formation, 20 m over
  every road the pass crosses and 33 m outside the straight, so they never
  cross the driving sight line. No jet wash reaches the karts or the
  camera.

## Consequences

- Every lap on Goldstone the jets come over the hangars as the driver runs
  down the Hangar Straight, with their roar.
- The jets follow the own kart only: in a room each driver's jets fly at
  their own time, and the karts ahead of or behind the own kart are not
  met by them.
- A slow kart, spun out in Chapel or on worn tyres, reaches the hangars
  after the formation has passed over; a kart on a star run reaches them
  before it does. The pass stays timed for a kart at racing speed.
- A layout change that moves Chapel or the hangars moves
  `GOLDSTONE_CHAPEL_M` with it; the timing assumes the jets' pass is
  centred on the Hangar Straight's middle, which the track tests also hold.
