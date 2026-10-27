# ADR 0037: The Instrument panel is drawn from the car

## Status

Accepted (2026-10-26). Supersedes the look of ADR 0023 (gold type in a
brown one-pixel outline, bevelled gold boxes) and of ADR 0024 (16x16
sprite grids outlined in the panel's brown), and ADR 0031's bevelled gear
box and square shift lights. What those records decide about content,
layout, notation, accessibility and reduced motion stands. Settles ADR
0034's open question on the panel's face: the Kenney pixel face stays.
Its octagonal start light for each count is superseded by ADR 0040.

## Context

ADR 0034 made chibi low-poly the base style for every asset, and ADR 0035
and 0036 built the car and its bare-headed driver in it: flat-shaded
facets, each panel a lit and a shaded tone of one colour, polygons where a
circle would be, and a palette of primary, secondary and accent. The
Instrument panel kept the 16-bit kart racer of ADR 0023, chosen before the
car existed. Next to the car its brown outline, its gold and its pixel
sprites read as a second game: the cone in the item slot is not the cone
on the road, and the tyre gauge's kart is not the car you drive.

`docs/mockups/asset-catalog.html` draws every panel readout as it ships
beside a redraw from the car, at the HUD unit over a mock Race View, in
each livery.

## Decision

- Readouts sit on plates: chamfered corners and three horizontal bands, a
  lit, a mid and a shaded tone of one colour, with the posters' hard ink
  drop under them. No outline anywhere on the panel.
- The type stays Kenney Mini Square, as on the posters: values white,
  labels a light grey, a 0.1 em ink drop. The brown outline `#3a1c0a` and
  the HUD gold `#fde97a` retire from the panel.
- Plates in carbon carry the stack, the gear, FINISH and the standings.
  The position plate is the local kart's primary; the local kart's own
  marks (lap flags, hubs) take its accent. Other karts appear in their key
  colour: their label plates and their map markers. Danger is the cone's
  hazard orange; the connection banner and the countdown lights are poster
  red; GO is go green.
- Polygons, not pixels: the shift lights are hexagons and the course
  map's markers octagons, each faceted lit to the top left like a wheel
  hub. The countdown shows each count on an octagonal start light.
- The tyre gauge is the local kart's car from above: wings in secondary,
  tub and sidepods in primary, the accent stripe, the halo and the
  driver's hair, rears bigger than fronts, each tyre a faceted block in its
  wear colour.
- The course map draws the track as the world builds it: asphalt between
  white walls over the ink drop, with a chequered start.
- The item slot is the item box seen face on, in the item box golds, which
  the View's item box mesh shares. The held item is drawn from the View's
  mesh for it, flat-shaded: the grumpy cone, the seagull, the snail with a
  rocket strapped to its shell. Firing lights the plate.
- Kart labels are a plate in the kart's key colour with a pointer down to
  the car.

## Consequences

- Presentation only. The sim, the protocol and the bridge are unchanged
  and the replay fixtures stand.
- Every readout keeps its `data-instrument` name, its text and the item
  slot's `data-item` and accessible name, so tests read the panel as
  before. The tyre gauge keeps `data-tyre-corner`.
- The item characters stop being string grids: `item-sprites.ts` becomes
  inline SVG polygons drawn from the meshes, and the View's cone, ball and
  item box become the lit, faceted meshes the catalog draws. One shape per
  item, in the slot and on the road.
- Plates add area the old no-box rule kept off the View; they stay sized
  in HUD units so the eight-window feel check still reads.
- The Room Scene and the favicon are out of scope and keep their own
  tickets.
