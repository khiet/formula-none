# ADR 0033: The Title and Results Scenes are a Grand Prix poster

## Status

Accepted (2026-10-26). Extends ADR 0023: it takes up the Title and Results
Scenes that ADR 0023 left in their old look, and loads its face before the
Title Scene rather than before the Race Scene.

## Context

The Title Scene and the Results Scene were browser-default forms on black.
Neither felt like a kart game, and finishing a race, even a win, got a
plain text list. Three directions were mocked up in
`docs/mockups/title-and-results-variations.html`; issue 104 picked design
A, Grand Prix Poster.

## Decision

- Both Scenes are set in the Instrument panel's pixel face on a red poster
  with slowly rotating rays and chequered bands rolling along the top and
  bottom. The face loads before the first Scene.
- Buttons are chunky with hard shadows that press in. Nothing pulses, and
  everything holds still under `prefers-reduced-motion`.
- The Title logo crosses out a "1" between Formula and None, under it the
  tagline "Kart racing for up to 8 friends". Create room is the green
  primary button; Join, beside the code field, and Solo race are yellow.
  The audio toggles are chips with their full labels.
- The Results Scene leads with a headline for your finish: "You win!",
  "Podium!" and the place, the place stamped large with the karts off the
  podium from 4th down, or "Out of road!" when you did not finish. Places
  1 to 3 stand on a podium as flat side-on karts in their livery; a board
  beside it lists the rest with a livery swatch each. A kart that did not
  finish reads "Not finished" in the best-lap column. The lowest best lap
  among the karts that finished gets a "Fastest lap" badge, on every kart
  that ties it, worked out on the client; a kart that did not finish never
  holds it, since its row shows no time. Confetti falls only for a finish
  from 1st to 3rd.
- The reveal is done within about 2 s and never holds back Rematch or
  Leave.
- Styles stay in `apps/web/index.html` under `#title-scene` and
  `#results-scene`, and the layout scales with the window, clean down to
  1024x640.

## Consequences

- Presentation only: no sim, protocol or audio change.
- Controls keep their names and rows keep `data-result`, so the e2e specs
  find them as before; only the not-finished text changed.
- The Room Scene keeps its old look until a decision takes it up.
