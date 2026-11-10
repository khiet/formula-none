# ADR 0105: The timing tower, the coin pile and the kart from behind

## Status

Accepted (2026-11-09). Supersedes ADR 0037's separate carbon plates for the
stack and its tyre gauge drawn from above, makes a rule inside a plate an
exception to its no-outline rule, and supersedes ADR 0056's coin readout,
`x07 / 10`. What those records decide otherwise stands: plates, type, the
gear, the item slot, the course map, the ink drop and the coin rule. Records a
design only; the build follows in its own issue.

## Context

The top-left stack holds four plates above the item slot: the position plate in
the livery and three carbon plates, lap, time and best, each sized to its text.
Four drops and four right edges draw a ragged outline down the left of the
View. Under it, the speed cluster reads the coins as `x07 / 10`, a times sign
borrowed from an item count, a zero-padded figure and the cap as a fraction,
and the tyre gauge draws the local kart from above, a view of the car the race
never shows.

Two mockups weigh three takes each, in the panel's palette, chamfer and type,
with the rest of the panel as it ships:

- `docs/mockups/stats-stack-variations.html`: the timing tower, only the
  position on a plate with the clocks as outlined type, and a pit board of
  digit tiles.
- `docs/mockups/coin-and-tyre-gauge-variations.html`, under the tower: ten
  pips, one stamped coin and a pile beside the gear for the coins; four tyres
  with no kart, the poster kart side on and the kart from behind for the tyre
  gauge.

The maintainer chose the timing tower, the pile beside the gear and the kart
from behind. The mockups left three things open: the tower's width, whether
the lines between its rows break ADR 0037's rule against outlines, and what
the coin readout's text says now that it draws its cap.

## Decision

- **The stack is a timing tower**: one carbon plate with one straight right
  edge. The position is a band in the local kart's primary set into its top;
  the lap, time and best rows under it share a label column, so their values
  line up. The position keeps no plate or drop of its own.
- **The tower is 26 HUD units wide on every race**, not the mockup's 24, so the
  lap row holds five lap flags, the most a race can have (ADR 0084). Its width
  does not change with the room's laps.
- **The hairlines between the tower's rows are rules, not outlines.** A rule
  divides one plate's rows inside it. ADR 0037's "No outline anywhere on the
  panel" stands for every edge of a plate and every glyph; a rule inside a plate
  is the one exception.
- **The coins are a pile beside the gear**: coins on edge stacked in a column
  at the speed cluster's left, one per coin carried, the empty slots up to the
  cap as faint lines, and the count above the pile. The gear plate moves right
  to make room.
- **The coin readout's text is the count**, `7`, with no times sign, padding
  or fraction. The empty slots show the cap, and the readout's accessible name
  says both, as in `7 of 10 coins`.
- **The tyre gauge is the local kart from behind**, as the chase camera sees it:
  the rear wing across the top, the two big rear tyres either side toned by
  ADR 0080's bands, the driver's head over the tub, in the livery. The
  percentage stays beside it.

## Consequences

- Presentation only. The sim, the protocol, the bridge and the replay fixtures
  are unchanged, and nothing is built until the follow-up issue.
- The build keeps every readout's `data-instrument` name and the lap count's
  text behind the flags, so tests read the panel as before. The coin readout's
  text changes from `x07 / 10` to the count, and its tests change with it.
- The tower covers more sky than today's lap plate, and the pile adds height at
  the bottom-left corner. Both stay sized in HUD units for the eight-window
  feel check.
- Only the rear tyres show in the gauge. The band is one colour for all four,
  so nothing is lost; a per-corner reading would need a new decision.
- `docs/style-guide.html` changes with the build, not with this record.
