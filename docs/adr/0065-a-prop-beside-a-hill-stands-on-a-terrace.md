# ADR 0065: A prop beside a hill stands on a terrace

## Status

Accepted (2026-10-31).

## Context

ADR 0050 stands every prop flat at the grass's level, because the only
hill was inside the walls and no prop had to read a height profile. Monarch
(ADR 0064) climbs 8 m from Sainte Devote to its crest at Casino Square, and
the mockup's scenery decision puts the casino outside the square, at the
crest. Stood on the grass, the casino's front would start 8 m below the road
beside it, so it would read as sunk in a pit rather than facing the square.

## Decision

- A prop that stands beside a hill may stand on a terrace: a stone block
  from the grass up to the road's height at the prop's spot on the lap, in
  `stand-plinth` under `beam-top` paving, covering the prop's footprint.
  The prop stands on the terrace's top.
- Only a prop placed by a `Spot` reads the road's height, since a spot is
  the only placement tied to the lap. The casino refuses a `World` place.
- The casino is the one prop that does this today. Every other prop still
  stands flat at the grass's level, as ADR 0050 decided.
- The terrace is scenery like the prop on it: it is in the glTF only, held
  to the prop's footprint and the 8 m rule, and never reaches the sidecar.

## Consequences

- The casino stands level with Casino Square's crest and reads from the
  chase camera as the house on the square.
- A terrace is a tall block beside the road, so the footprint's 8 m rule
  keeps it as clear of the walls as the building on it.
- The rule "a prop reads no height profile" in ADR 0050 is superseded for a
  prop on a terrace; the rest of ADR 0050 stands.
