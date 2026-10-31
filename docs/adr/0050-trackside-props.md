# ADR 0050: Trackside props are scenery the layout declares and the View moves

## Status

Accepted (2026-10-29). Its rule that every prop stands flat at the grass's
level is superseded for a prop on a terrace by ADR 0065.

## Context

Suzuqa Patch is a road through empty grass. The stands, hoardings, gantry
and bridge dress the road itself; nothing fills the green space between the
legs, and no corner has a landmark. The design for what should,
`docs/mockups/trackside-objects.html`, proposes woods along the back
section, a Ferris wheel by the first corner, an arch and a pit building,
and an airship. Each is scenery, and two of them move. The sim reads the
sidecar and never the glTF (ADR 0007), and the replay fixtures under
`assets/replays` hash everything the sim steps, so anything that reached
the sim would re-record every fixture.

## Decision

- A prop is scenery a layout declares in its `Decor` and the builder
  places; it lives in the glTF only, and the sidecar is unchanged by it.
  Karts never collide with a prop. A prop that must stop a kart becomes a
  wall or a tyre wall in the sidecar, with its own ADR.
- A wood is a patch of trees the builder scatters from a kind, a patch, a
  count and a seed. The count is a cap: a tree within 3 m of a wall, canopy
  and all, or 5 m of another tree is dropped, so a layout never places a
  tree by hand and a patch near a wall gets fewer. Each wood's seed starts
  its own random stream, which also turns each tree to its own angle, so
  the same layout exports the same trees. Every third round tree is young.
- Every prop stands flat at the grass's level. The only hill is inside the
  walls and the bridge's embankment is a vertical face at the walls' foot,
  so a prop reads no height profile. A wood that reaches past the grass
  slab's edge draws its own grass beneath it, scenery a hair under the
  slab, since the slab is a surface in the sidecar and never grows for a
  prop.
- A moving prop is moved by the View from its own clock, as the marshal's
  flags are: Presentation, with no state in a snapshot. Two clients' wheels
  need not agree.
- Props carry lettering only from `trackgen/sponsors.py` (ADR 0038). Real
  Suzuka landmarks are borrowed as kinds of thing, never copied (ADR 0007).
- A prop is drawn in the chibi low-poly manner (ADR 0034): flat facets in a
  lit and a shade tone of one colour, no textures, one mesh per material
  per family.
- No handling of `prefers-reduced-motion`, as for the marshal and the karts.
- The oval gets no props: it is the fixture every sim test loads.

## Consequences

- The tree builders live in `trackgen/props.py`, one per kind, and each
  later prop family joins them there; `layout.py` holds the `Decor` fields
  and the clearance rules.
- A change to a prop re-exports the track's glTF and leaves the sidecar
  byte-identical; the track tests hold every tree to its clearance and
  spacing and scatter each wood twice to the same trees.
- The woods are the first slice. The Circuit Wheel, the arch and pits, and
  the airship follow, each with its card in the style guide.
