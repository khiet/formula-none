# ADR 0106: Monarch narrows at the hairpin and Rascasse

## Status

Accepted (2026-11-10). Supersedes ADR 0064's 12 m of asphalt the whole way
round Monarch. Its 2 m of run-off stands.

## Context

Monarch's asphalt is 12 m everywhere (ADR 0064). The real Monaco runs about
8 to 12 m: about 12 m on the start/finish straight, about 7 to 8 m at the
Grand Hotel hairpin, about 9 to 10 m from the tunnel exit into the chicane.
Card M5 in `docs/mockups/course-excitement.html` asked for road that is wide
between tight corners; issue 360 gets that contrast by narrowing the tight
corners toward Monaco's own widths instead of the card's 15 m harbour pocket,
which has no real basis.

The sim reads the sidecar's asphalt, sand and wall polygons and holds no
width, so this is the track generator's alone.

## Decision

- A layout keeps `asphalt_half_width` as its default and may name segments
  with a half width of their own (`half_widths`), joined by `Taper(s_start,
  s_end)`s: linear in s from the width before the taper to the width after
  it, so the edge never steps. `Layout.half_width_at(s)` is the one reading;
  the asphalt, the sand, the walls, the painted kerbs, the checkpoints'
  reach, the hoardings, the gantry, the start line and a wheel crossing all
  follow it. The run-off keeps its width beyond the local edge, so the walls
  follow the road in.
- A tunnel, a bridge, a ramp, a pit lane and a wheel crossing's hazard zone
  are built to one width, so the export refuses a layout whose width changes
  under one.
- Monarch, by half width (full width in brackets):
  - Mirabeau, the kink, the hairpin approach and exit: 4.5 m (9 m).
  - The Grand Hotel hairpin: 4.0 m (8 m), closing over its first 5 m and
    opening over its last 5 m.
  - Mirabeau Bas: 5.0 m (10 m), opening over its first 5 m.
  - La Rascasse: 4.5 m (9 m).
  - Everywhere else: 6.0 m (12 m), the default.
- The tapers: in over the last 15 m of the Casino descent, after the wheel
  crossing's hazard zone; out over Portier and its approach, so the tunnel
  entry and its box row have the full width; in over the last 15 m of the
  quay, after its box row; out down the Rascasse exit straight, back to 12 m
  about 30 m before the pit lane's fork.
- Mirabeau Bas was 9 m like the rest of the zone until the bot gate, the
  200 races of eight bots at each level, failed on it: a boosted Basic bot
  cleared its wall and flew past Portier's checkpoint outside its reach, a
  lap lost. Widened by 0.5 m of half width, every race finished.

## Consequences

- Every kart finishes in 200 races of eight bots on Monarch at every level.
- `monarch-lap-with-bots` is re-recorded on the narrowed road.
- No other course's export changed: a layout without overrides reads its
  default everywhere.
- Nothing in the generator limits how steeply a taper closes the road; the
  tests hold Monarch's to 0.1 m of half width per metre at most.
