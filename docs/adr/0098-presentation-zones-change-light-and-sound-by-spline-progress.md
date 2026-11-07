# ADR 0098: Presentation zones change light and sound by spline progress

## Status

Accepted (2026-11-07).

## Context

Light, sound and enclosure stayed the same the whole lap on every course.
Monarch's tunnel has a roof and lamps, but the sky's light reached the road
under it, the engine sounded as it did in the open, and driving out into
the harbour was not a moment. Several course ideas want the same change of
place (`docs/mockups/course-excitement.html`, cards X4 and M2): Bell Deer's
underpass, Goldstone's hangar and Monarch's tunnel to harbour. Bell Deer's
bridge puts two legs of the lap at one place on the ground.

## Decision

- A **presentation zone** is a stretch of a course in metres of spline
  progress from the start line, with four edges: it eases in from the
  first to the second, holds its whole look to the third and eases out by
  the fourth, each ease a smoothstep. A zone may run over the start line.
- Wholly in, a zone sets a scale on the sky's light, the share of the
  engine sent to its echo, overhead lamps by their first progress and
  spacing, a level for each ambient bed and a level for the crowd. Where
  two zones overlap, their light scales multiply and the louder of each
  other part wins. Outside every zone the blend is the open sky: the
  course's own light and no ambience.
- `apps/web/src/presentation-zones.ts` is a pure module: the zones of each
  course, and the blend at the own kart's spline progress, which the Race
  Scene hands to the View and the audio every frame. It reads the
  progress the standing reports, never a position, so at Bell Deer's bridge
  the leg the kart is on decides.
- The zones are a client table per course, not sidecar data. The sim never
  reads a zone, so the sidecar, the sim and the replays are untouched. The
  table's edges are the layout's (`assets/tracks/trackgen/layouts`) and are
  named after the segments and props they come from; a layout change that
  moves one moves the table with it.
- Monarch has two zones:
  - **the tunnel**: in across the mouth's portal, 10 m either side of the
    roof's start, out from 10 m short of the roof's end to the start of
    `chicane-in`. Light at 0.45 of the sky's, the engine's whole echo and
    the lamps every 12 m from half a spacing under the roof, as the layout
    hangs them.
    The dark follows the roof, not the whole of `tunnel-entry`, so the road
    is never dim under open sky.
  - **the harbour**: in from the roof's end to the start of `harbour`,
    held along it and out by the end of `tabac`. Light at 1.15 of the
    sky's, the harbour bed whole and the crowd at 0.6 for the harbour's
    grandstand.
    It starts at the roof's end, inside `tunnel`, so the bright rises as
    the dark lifts, and eases out over `tabac` rather than cutting off at
    the end of `harbour`.
- The View scales its sun and ambient light by the blend's light, warms the
  ambient towards the lamps' colour by their glow, and hangs a warm point
  light at the lamps' height over the own kart, as bright as their glow.
  The glow swells from lamp to lamp and never falls below half its peak,
  so the lamps pass without strobing.
- The audio sends the engine into a short feedback delay, its slap off the
  walls, by the blend's echo, gliding in and out over about half a second.
  The harbour bed and the crowd are loops `assets/audio/build.py` renders,
  levelled by the blend and never pitched, under the SFX toggle with the
  other loops.

## Consequences

- On Monarch the driver goes into a darker, echoing tunnel with the lamps'
  light passing over the kart and comes out into a brighter harbour with
  the water and the stand.
- The darkest the tunnel gets is 0.45 of the sky's light, lit further by
  the lamps, so the karts, the drones and the chicane stay readable, and no
  edge changes the light by more than a few per cent a metre, so the exit
  is no flash.
- The zones follow the own kart only: other karts are lit as the own kart
  is, so a kart behind in the open looks dim while the own kart is under
  the roof. The sky through the portals keeps its colour.
- Bell Deer, Goldstone and the oval have no zone yet; each is a table entry
  away.
