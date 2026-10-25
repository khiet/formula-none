# ADR 0023: The Instrument panel speaks 16-bit

## Status

Accepted (2026-10-25). Extends ADR 0007. Its decision on where the greybox
readouts sit, speed in the stack and the surface and connection line
bottom-left, is superseded by ADR 0031.

## Context

ADR 0007 settled the art: Kenney CC0 kits for the karts and the track,
everything under `assets/` CC0, and the Instrument panel as the HUD that
reads state and never drives it. It said nothing about how the panel
looks, and the panel shipped as a greybox: one monospace line in a dark
box, every readout at the same weight. It told the driver where they
stood but had no voice, and the Results Scene and later Scenes had nothing
to inherit from.

Five treatments were compared on a mock Race View, kept as
`docs/mockups/instrument-panel-variations.html`: a 16-bit kart racer, a
broadcast timing graphic, an arcade cabinet, a minimal editorial layout
and a rally co-driver sheet. The 16-bit look won: outlined pixel type on
the View with no box, laps as checkered flags, a bevelled item box. It
suits a game whose karts hop and drift, and it is cheap to extend.

## Decision

- The client's HUD language is the 16-bit kart racer: pixel type set
  straight on the View, gold with a one-pixel outline in the face's own
  grid and a two-pixel black drop, label words white and values gold, no
  box behind the readouts, no scanlines. Later Scenes follow it where they
  show race state.
- The type is a CC0 pixel face from Kenney's font pack, vendored under
  `assets/fonts/kenney` with the pack's own `License.txt` and a row in
  `LICENSES.md`, like the kits. No OFL font and no third-party font host:
  the client ships it and registers it with the FontFace API before the
  Race Scene mounts. A pixel typeface is a style, not a Nintendo
  character, circuit or trade dress; ADR 0007's line holds.
- The position is a large numeral with a small ordinal suffix, gold like
  the rest and not the kart's livery. Laps are a LAP label beside one
  checkered flag per lap of the race, filled as laps are completed and all
  filled on finishing; the lap count stays in the text for assistive
  technology. The item slot is a bevelled box: empty is an empty box, held
  shows the item's name, firing flashes.
- Times print in the racer's notation at the sim's precision, 0'27"304,
  with -'--"--- for a time there is none of yet. The notation lives in one
  formatter, so the panel and the Results Scene change together.
- The countdown, GO, the wrong-way warning and the connection banner take
  the same type at their sizes and positions; the warning blinks. The
  blink and the firing flash hold steady under `prefers-reduced-motion`.
- Type scales with the viewport, vmin based with a floor, so the HUD reads
  the same in an eight-window feel check as full screen.
- The greybox readouts stay: speed in the stack in small type, surface and
  the connection status in a dim line bottom-left, under their
  `data-instrument` names, so the e2e specs and the smoke recipes keep
  their hooks.

## Consequences

- Presentation only. The sim, the protocol and the bridge are unchanged
  and the replay fixtures stand.
- Every readout keeps its `data-instrument` name and its text, so tests
  find the panel the same way; only the time notation changed what they
  read.
- Laps per race is a tuning constant of three. A configurable count, and
  what the flag row does past a handful, is a follow-up.
- The course map, the kart labels and the Title, Room and Results Scenes
  beyond the shared time notation are out of scope and keep their current
  look until a decision takes them up.
- The variations page is the record of what was compared. It loads its
  fonts from Google Fonts for the comparison and ships nowhere.
