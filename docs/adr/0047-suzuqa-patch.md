# ADR 0047: Suzuqa Patch, a Suzuka figure eight raced over two laps

## Status

Accepted (2026-10-29). Supersedes ADR 0014's three laps, and ADR 0041's
places for the sausage kerb and the runaway wheel's crossing on Brands
Patch. Its two laps are superseded by ADR 0054, which restores three;
the course stands.

## Context

Brands Patch was an 800 m counter-clockwise loop with two hairpins, a hill
and a chicane. The game wanted a race track players would recognise, and
Suzuka is the figure eight most fans know. A figure eight crosses itself,
which the sim could not race until spline progress was searched near the
last reading (ADR 0046). The design, with every placement, the elevation
profile and the engineering notes, is `docs/mockups/suzuka-course.html`.

## Decision

- Brands Patch is rebuilt in place as Suzuqa Patch. It keeps the
  `brands-patch` id, its sidecar and glTF paths and its sponsor jokes;
  players see no track name, so the new name reaches only docs and
  comments.
- The corners follow the real Suzuka as closely as kart scale allows: the
  first curve, turn 2, the esses, Dunlop, the two Degners, the hairpin,
  200R, Spoon's two apexes, the back straight, 130R, the chicane and the
  final curve. Each corner's direction and heading change were measured
  from the outline in `bacinger/f1-circuits` `jp-1962.geojson` (MIT). No
  data from it ships, so `assets/LICENSES.md` is unchanged.
- Fidelity wins over lap length: the lap is 1306 m, against 797 m, so a race
  drops from three laps to two (`LAPS_PER_RACE`), about 2610 m in all.
- The back straight crosses the run from Degner 2 to the hairpin on a
  bridge 5 m high, near square. A smooth hill of 110 m carries the road up
  and down; the underpass stays at ground level. The bridge's embankment,
  its abutments either side of a span 30 m wide, and the deck's sides and
  underside are scenery in the glTF only. The sidecar is unchanged by
  them: the raised road's own asphalt and sand carry the karts, the ribbons
  have only a top face, and the colliders ignore back faces, so a kart in
  the underpass never meets the deck.
- Checkpoints, item boxes, boost pads, kerbs, tyre walls, the grid and the
  wheel's path each stand at least 20 m in plan from the other leg at the
  bridge, since the sim reads them by position alone. Fifteen evenly spaced
  checkpoints put the nearest 24 m from the crossing on either leg; a
  trackgen test holds the rule.
- Tyre walls stand inside the hairpin and Spoon's second apex and outside
  turn 2. Turn 2 turns right, and every tyre wall stands left of the bots'
  line, so the draft's wall inside it moved outside.
- Sausage kerbs sit inside esses 3 and the chicane's right-hander. The
  runaway wheel crosses the run to the hairpin 28 m past the bridge, its
  marshal post on the outside of the hairpin, to the right.
- The course map draws the deck over the road beneath it, so the crossing
  reads as a bridge rather than an X.
- The hairpin lies in line with the wheel's crossing, 50 m past it, so a
  bot turning through it faces back toward the wheel. A bot facing back
  toward a crossing that it is past along the lap by more than the
  wheel's reach and its lookahead waits for no pass. The track keeps each
  crossing's spline progress for this, found over the whole lap when it is
  built, as a checkpoint's is.

## Consequences

- A race takes longer at the same pace: a lap about 1.6 times Brands
  Patch's, two of them instead of three.
- Every test that read Brands Patch's geometry, and both of its replay
  fixtures, were moved to the new course in the same change. The fixtures
  keep their names: `brands-patch-first-corners` runs four karts from the
  grid through the first curve and turn 2, and `brands-patch-chicane` one
  kart from the grid, spun out on the wheel's crossing, over the bridge
  and the chicane's kerb to the line.
- A kart placed by hand within a few metres of the bridge reads its first
  spline progress over the whole lap, and so may read the other leg. The
  grid and every checkpoint a kart respawns at stand clear of it, so only
  a test that parks a kart there needs care.
- A new course that crosses itself needs the same clearance at its bridge,
  and a checkpoint count that keeps the gates off it.
