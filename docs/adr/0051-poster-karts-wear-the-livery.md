# ADR 0051: The poster kart is the View's car, and wears its livery everywhere

## Status

Accepted (2026-10-30). Supersedes the clause of ADR 0035 that listed the
Title Scene's runners among the key colour's uses.

## Context

ADR 0033 put a flat side-on kart on the Grand Prix poster, and ADR 0035
painted it in the livery on the podium but in the one key colour as a
Title Scene runner. ADR 0034 then rebuilt the car the View draws as a
chibi low-poly F1 car from primitives in `assets/karts/build.py`: a short
wheelbase, oversized octagon wheels with three spokes, a big bare head.
The poster kart kept the older mockup's proportions, a long low car on
small wheels with an accent hub, so the Title Scene showed one car in one
colour, the Results Scene another car in three, and the race a third.
`docs/mockups/poster-kart-alignment.html` sets the three side by side.

## Decision

- The poster kart (`apps/web/src/poster.ts`) is `build.py`'s car projected
  onto its side: every box in the same metres, painted far side first so
  each nearer part covers what it hides in the View, the wheels and the
  head placed where the build puts them. A change to the car's shape in
  `build.py` is a change to the poster kart in the same PR.
- The poster kart wears the whole livery, primary, secondary and accent,
  wherever it is drawn: the Title Scene's runners, the Room Scene's grid
  and the podium. The key colour keeps its other single-colour uses.
- Each wheel is a group of its own, `.wheel`, turning about its centre.
  Where a poster kart moves, its wheels turn with the ground it covers,
  forwards as the View turns the car's: the Title Scene's runners roll
  under the same motion preference as their run.

## Consequences

- The side view is 520 x 380 rather than 520 x 260, so a kart of a given
  width stands taller: the Room Scene's rows grow to hold it.
- `docs/style-guide.html` draws the same projection; the guide's side view
  of the car is the poster kart.
- A wheel's turn is a style, not a readout: the runners' speed is the
  poster's own, not a sim speed.
