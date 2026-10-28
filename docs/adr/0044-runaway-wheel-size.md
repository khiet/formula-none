# ADR 0044: The runaway wheel is 1.2 m in radius

## Status

Accepted (2026-10-28). Supersedes ADR 0041 on the wheel's size, which it
left to tuning.

## Context

ADR 0041 put a runaway wheel on the approach to Brands Patch's second
hairpin and left its size to `WHEEL_RADIUS_M`, which shipped at 0.45 m.
At 0.9 m across the wheel is about a kart's width: from the chase camera it
reads as debris rather than a danger, and at 15 m/s it holds a lane for
about 0.14 s. The design spec, `docs/mockups/runaway-wheel-danger-spec.html`
(the Decisions table's Wheel size and the Hot wheel section's Reach), grows
it to 1.2 m.

## Decision

- `WHEEL_RADIUS_M` is 1.2 m: the wheel is 2.4 m across, a fifth of the
  12 m asphalt, and holds a lane for about 0.24 s.
- The wheel drawn stays the wheel that hits. A kart whose box comes within
  `WHEEL_RADIUS_M` of the wheel's centre spins out, by no one; the View
  scales today's mesh from the same value (0.96 m wide at the 0.8 width
  share). The hot wheel look is a separate decision.
- The rebound point follows the radius, the centre `WHEEL_RADIUS_M` off the
  wall, so legs shorten: a pass on Brands Patch lasts about 4.5 to 5.3 s,
  and with its 1 s flag still fits `WHEEL_PERIOD_S`.
- A bot's avoidance box, reach plus half the kart's length and width,
  grows with the radius. `BOT_WHEEL_MARGIN_S` stays 0.3 s.

## Consequences

- In the 40-seed sweep of eight bots racing three laps of Brands Patch,
  bots touch the kerb in one seed, as after ADR 0043, and none is hit by
  the wheel or misses the end cap.
- Over 200 seeds the wider pass hits a bot crossing slowly at the lane's
  edge in one seed, and bots touch the kerb in 6. Margins of 0.2 to 0.4 s
  each leave a seed or more in 200 with a hit or a missed cap, so the
  margin is left where it was.
- The wheel's passes change, so the Brands Patch chicane fixture was
  re-recorded; the fixtures that never meet a pass are unchanged.
