# ADR 0052: The poster kart is generated from kart.glb

## Status

Accepted (2026-10-30). Supersedes the consequence of ADR 0051 that the
poster kart is `build.py`'s car traced by hand, part for part, in
`poster.ts` and again in the style guide; what ADR 0051 decided the poster
shows, and where, stands.

## Context

ADR 0051 made the poster kart the View's kart and put it in the whole livery
on the Title runners, the Room grid and the podium. It got there by tracing
`assets/karts/build.py`'s boxes by hand into `apps/web/src/poster.ts`, and
copying the tracing into `docs/style-guide.html`. Three drawings of one kart
meant a change to the build was three changes, and nothing failed when the
poster fell behind the glb: the rim and tyre greys already disagreed, and
the head was drawn once as a side-on circle and once as a faceted shell.
`docs/mockups/poster-kart-generated.html` sets the race render, the tracing
and a projection of the glb side by side for all eight liveries.

## Decision

- The poster kart is projected from `assets/karts/kart.glb`, the export the
  race loads, by `tools/kart-poster`, a TypeScript package Node 22 runs as
  `pnpm generate:poster`. It parses the glb itself; no Three.js, no Blender.
  `pnpm export:karts` runs it after Blender, so the one command refreshes
  the glb and the poster together, and CI regenerates the poster and fails
  on a stale copy beside the protocol types.
- The projection is an orthographic side view from the kart's right, nose
  to the right, pitched down 4 degrees so every top face shows as the lit
  strip the tracing drew along each box and the far wheels as the sliver of
  tyre an elevated side view gives. Back faces are culled, faces wholly
  hidden by a part always drawn with them are dropped, and the rest go in
  painter order from what overlaps what on the picture: one path per node,
  material and tone, split only where a face is both behind and in front
  of another. Nothing is culled by node name.
- Each face is shaded by its normal against one fixed light, quantised to
  lit, mid and shaded. The tones are not colours in the data: each path
  names its material and tone, `poster.ts` and the guide paint it from a
  custom property, and `index.html`'s `color-mix` percentages make the
  lit and shaded tones of every colour, livery and fixed alike. The fixed
  colours are `build.py`'s materials as the glb exports them, copied into
  `palette.ts` as `KART_PAINT` and held to the glb by `kart-asset.test.ts`;
  the rim and tyre greys the tracing had drifted on follow the build.
- The frame is the tracing's: 250 units a metre in a 520 x 380 viewBox, the
  kart's origin at x 262, its near wheels on the ground line at y 350, so
  the Room rows, the podium widths and the runners' CSS do not move.
- The generated data is committed twice from one run:
  `apps/web/src/generated/kart/side.json` for the web app to import, and
  `docs/kart-side.js`, the same data as a global, which the style guide
  loads with a relative script tag so it still opens from disk. Each
  carries the paths in painter order, every part's node name, and each
  wheel node's projected axle.
- `kartSide(livery)` wraps the paths in one `<g>` per run of a node, keeps
  only the parts the driver wears, and makes each wheel's group `.wheel`
  turning about its axle, so the Title runners' spin is unchanged. Which
  parts a driver wears is `driver-parts.ts`, which the View and the poster
  both call. The shadow ellipse and the backdrop stay hand-drawn;
  `poster.ts` holds no other geometry and no colour of its own.

## Consequences

- A change to the kart in `build.py` is one change: export, and the poster
  follows. A poster that looks wrong is fixed in `build.py`, not redrawn.
- The poster is what the glb shows side on. The eyes, boxes on the head's
  forward flat, show as a sliver of white and iris rather than the round
  eye the tracing drew; the light stubble, one forward-facing face on the
  chin, shows nothing at all. These follow the build until the build
  changes.
- All four wheels are groups, so a runner has four `.wheel`s, and a wheel
  keeps every face it has, hidden at rest or not, since a face behind the
  near wheel comes round as it turns. `index.html`'s wheel rule moves from
  the group's own box to the axle the data gives (`transform-box:
  view-box`), its front-wheel rule picks the wheel by node rather than by
  order, and it mixes lit and shaded tones for the fixed colours too; the
  runners' motion is otherwise as it was.
- A kart is at most 120 path elements, held by `poster.test.ts`; today it
  is about forty. The guide's chase view is still drawn by hand; the
  projector takes a camera, so it can follow in its own issue.
