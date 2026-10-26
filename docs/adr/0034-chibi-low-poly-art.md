# ADR 0034: Chibi low-poly is the art direction

## Status

Accepted (2026-10-26). Supersedes ADR 0007's choice of the Kenney car and
racing kits as the look of the game; the rest of ADR 0007 stands. The
"no driver resembles a real person" bullet is superseded by ADR 0035. The
oversized helmet and polygon helmets are superseded by ADR 0036: drivers
race bare-headed. ADR 0037 brings the Instrument panel into this style and
keeps the Kenney pixel face.

## Context

ADR 0007 put Kenney's CC0 kits in after M1 so the game had art without an
art task. The kart is Kenney's, recoloured to a livery, and its driver is a
blob with no face. The tracks are already flat colours from Blender with
two Kenney pieces, the grandstand and the gantry, for scenery.

The fiction is a cartoon parody of Formula 1, and the kit kart reads as a
go-kart, not an F1 car. Three directions were drawn in
`docs/mockups/driver-and-car-designs.html`: chibi proportions, real F1
proportions, and a faceted low-poly F1 car. Real proportions make the car
longer than the box the sim collides with, so the sim, the tuning and every
replay fixture would change, and realistic faces sit badly beside flat
tracks. The chosen style, chibi proportions built from low-poly facets, is in
`docs/mockups/chibi-low-poly-f1.html`.

## Decision

- Chibi low-poly is the base style for every asset: the cars, the drivers,
  the tracks and their scenery, and the Title and Results Scenes.
- Chibi: short wheelbase, oversized wheels and helmet, big-eyed drivers with
  a short jaw. A car fits the box the sim collides with today.
- Low-poly: flat-shaded facets, each panel in a lit and a shaded tone of
  one colour, flat materials and no textures. Wheels and helmets are
  polygons, not smooth circles.
- Cars are open-wheel F1 cars with wings, sidepods and a halo, built in
  Blender from primitives by a script, the way `assets/karts/build.py`
  builds the kart today.
- Drivers share one head; skin, hair and facial hair are what tell them
  apart. The same head is the portrait in the Room and Results Scenes.
- Liveries borrow a team's colours only. No team name, crest, sponsor or
  race number, and no driver resembles a real person.

## Consequences

- The Kenney kart and the Kenney grandstand and gantry are replaced as each
  asset is rebuilt; until then they stand in. Assets made here stay CC0 with
  a row in `assets/LICENSES.md`.
- A car painted in several colours does not fit the glossary's livery, one
  fixed colour per kart. That change gets its own ADR when the cars are
  built.
- Whether the Instrument panel keeps the Kenney pixel face of ADR 0023 is
  left to the Title and Results work.
- The sim consumes track data, never meshes, and the cars keep today's box,
  so the restyle changes no rules and no replay fixture.
