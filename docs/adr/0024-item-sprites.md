# ADR 0024: The item slot shows a character, not a word

## Status

Accepted (2026-10-25). Extends ADR 0023. Its 16x16 sprite grids and brown
outline are superseded by ADR 0037; the characters, the slot's states and
its accessible text stand. Its seagull for homing is superseded by ADR
0057: the homing character is a camera quad drone. Its snail for boost is
superseded by ADR 0058: the boost character is a podium champagne bottle.

## Context

ADR 0023 gave the Instrument panel its 16-bit voice and left the item slot
printing the held item's name: Boost, Hazard, Homing. A word in a bevelled
box reads, but it has no character, and a driver glancing down mid-corner
reads a picture faster than a word.

Five icon sets were compared on a contact sheet, each set also mounted in a
copy of the slot, kept as `docs/mockups/item-icon-sets.html`: 16-bit
sprites, flat stickers, a pit-wall blueprint, a cast of roadside pests and
the tyre marks each item leaves. The 16-bit sprites fit the panel's
language and cost nothing in style debt; the pests were the only set with
a face. The decision is both.

## Decision

- The held slot shows the item's sprite: a snail with a rocket strapped to
  its shell for boost, a grumpy traffic cone for hazard, a seagull with its
  eye locked on for homing. The cone agrees with the View, which already
  draws the hazard as a cone.
- Each sprite is a 16x16 grid. What survives the shrink is the face and the
  one prop: eyes plus rocket, eyes plus stripe, eye plus beak. Detail that
  does not fit at that size is dropped, not squeezed.
- The outline is the panel's outline brown; at most eight colours per
  sprite, drawn from the panel's gold and white, the hazard cone's orange,
  livery red and a grey. No colour that reads only against the slot's gold.
- The sprites are string grids in one TypeScript module under
  `apps/web/src`, one character per pixel through a named palette,
  rendered to an inline SVG of rects with `shape-rendering: crispEdges`.
  Diffable in review, no binary asset, no build step. They are original
  work under CC0, with a row in `assets/LICENSES.md` (ADR 0007).
- The slot: empty stays an empty box. Held shows the sprite at about two
  thirds of the box, scaling with the HUD unit; non-integer pixel sizes are
  accepted with `image-rendering: pixelated`, as the panel's type already
  scales. Firing keeps the sprite visible and flashes the box behind it,
  steady under `prefers-reduced-motion`. The characters have no idle
  animation.
- The item's name stays as the slot's accessible text, visually hidden; the
  sprite is `aria-hidden`. The slot carries a `data-item` attribute naming
  what it holds.

## Consequences

- Presentation only. The sim, the protocol and the bridge are unchanged
  and the replay fixtures stand.
- Tests and assistive technology keep reading Boost, Hazard, Homing from
  the slot; the sprite is found by `data-item` and the SVG under it.
- The View's cone, ball and item box meshes, the course map, and a fourth
  character for the item box are out of scope. The slot never shows the
  item box. Making the View match the characters is a follow-up.
- The comparison page is the record of what was compared. It loads its
  fonts from Google Fonts and ships nowhere.
