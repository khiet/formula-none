# ADR 0040: Five start lights replace the 3-2-1 count

## Status

Accepted (2026-10-27). Supersedes ADR 0037's line "The countdown shows
each count on an octagonal start light"; the rest of ADR 0037 stands.

## Context

The countdown showed 3, 2 and 1 on one octagonal plate. Issue 137 swaps
the count for an F1-style start: one light per second, all out at Go.
`docs/mockups/start-lights.html` compares three treatments: A, bare
poster-red lamps; B, the same lamps on a carbon gantry; C, the lamps with
the count printed on them. A reads fastest at the size of an eight-window
feel check and keeps the panel's one rule that a plate is a readout.

## Decision

- The countdown lasts 5.0 s (`COUNTDOWN_S`). The first light comes on at
  5 s before Go and the fifth at 1 s before; all five go out at Go. There
  is no random hold before lights out: timing by ear and the fixed rocket
  start windows depend on Go coming 1 s after the fifth light, and the sim
  takes no new seeded draw.
- The rocket start's hold edges stay where they were in seconds back from
  Go, so they now fall in the fourth light (full boost in its first half,
  weak boost in its second) and the fifth (clean); a hold begun before the
  fourth light stalls.
- Treatment A: the Instrument panel draws a row of five octagonal lamps
  centred 40% down the View, each 5 HUD units square and 1 HUD unit apart.
  A lit lamp is poster red; an unlit one is `LAMP_OFF`, carbon one step
  darker, and drops no shadow. At Go the lamps give way to the GO plate in
  go green for a second, as before.
- The panel always draws five lamps; a Vitest test holds the tuned
  countdown to one second per lamp. The lamps lit are read from the same
  count the audio beeps on, so one beep per lamp and the go cue at lights
  out.

## Consequences

- A room's Countdown state is 2 s longer, and every race starts 120 ticks
  later, so the replay fixtures were re-recorded.
- The row's test hook is `data-lit` (0 to 5) on
  `[data-instrument="countdown"]`; GO keeps its text.
- A 3D gantry over the grid, a random lights-out delay and numerals on the
  lamps stay out of scope.
