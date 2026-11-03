# ADR 0080: Tyres wear, and the gauge reads their life

## Status

Accepted (2026-11-03). Supersedes ADR 0056's tyre gauge holding at 100%,
and ADR 0055's consequence that the gauge does not read wear. Coins and
their rule stand. The specification and its evidence are in
`docs/mockups/tyre-wear-and-pit-lane.html`.

## Context

The tyre gauge has held at a placeholder since ADR 0031. ADR 0055 dropped
wear because the gauge needed something to read and wear had never been
built, not because wear had failed. A kart that wears its tyres gives a
driver something to manage over a race: how hard to corner, when to drift,
and whether to stop for new ones (ADR 0081).

F1's wear and degradation are different things, with reversible heat, four
separate corners and several compounds. A race here lasts under five
minutes, so one number a player can read has to do all of that work.

## Decision

- A kart has one `tyre life` for all four tyres, from 100% for new to 0%.
  It falls and only a stop raises it (ADR 0081). Heat, compounds and
  random punctures are out: a kart that loses a race should see why.
- These wear the tyres, all deterministic, with constants in
  `crates/sim/src/tuning.rs` (ADR 0014):

  | Source | Life lost |
  | --- | --- |
  | Rolling on the ground | 0.06% / s |
  | Cornering | 0.19% / s x (a / 20)^2, a being speed times yaw rate in m/s^2 |
  | Drifting | 1.5% / s, on top of cornering |
  | Mini-turbo release | 0.5% each |
  | Braking | 0.25% / s, scaled by the brake input |
  | Grass or sand | 1.25% / s |
  | Spin-out, whatever caused it | 6% each |
  | Bump against a kart or tyre wall | 0.3% + 0.05% per m/s of closing speed, at most 2.5% |
  | Held against a wall | 1% / s |
  | Sausage kerb launch | 1% each |

  Nothing wears a kart in the air, on a star's run, spun out or stopped.
- Worn tyres cost speed and grip, by these factors, interpolated linearly
  between the columns:

  | Effect, at life | 100% | 60% | 30% | 0% |
  | --- | --- | --- | --- | --- |
  | Top speed cap | 1.00 | 0.99 | 0.96 | 0.86 |
  | Steering rate | 1.00 | 0.97 | 0.88 | 0.65 |
  | Acceleration | 1.00 | 0.97 | 0.88 | 0.70 |
  | Braking | 1.00 | 0.98 | 0.90 | 0.75 |
  | Wash-out, most slip | 0 | 0 | 0.05 rad | 0.15 rad |

  The cliff at 30% is deliberately more dramatic than current F1: a short
  race needs a warning a player can read. Handling had no lateral grip, so
  wash-out is new: the line of travel slides wide of the facing past a
  cornering limit, with no random twitches. Mini-turbo charge times do not
  change. At 0% a kart still reaches 93 km/h and can always finish.
- Life and its factors live in `Handling`; the caller hands in the facts
  through `Situation`, so handling reads neither the track nor the physics
  world (ADR 0027).
- The tyre gauge reads life, rounded up so 0% means truly gone. Its bands
  stay at 60% and 30%, so red and the cliff start together. A BOX plate and
  chime show for 2 s when life first falls to 30%, not on a course without
  a pit lane nor on the final lap. Karts below 30% trail light tyre smoke
  and darker tyre marks, so rivals can read each other.
- A kart's prediction wears its own tyres, since that follows from its
  inputs. Wear from other karts and hits arrives by snapshot, as coins do.
  A 1% disagreement moves the top speed cap by at most 10 cm/s, too little
  to see.
- Bots wear by the same rules. Their steering shares the same rate, so they
  slow for corners on worn tyres.

## Consequences

- Nothing here is built yet: until it is, the gauge keeps its placeholder.
- Every rate and factor is a starting tune. Wear was measured by applying
  these rates to bots on today's handling; lap costs come from a speed
  profile model; neither has been raced. Ace bots lose 32 to 47% a lap on
  Bell Deer, which should be retuned against playtests.
- A hit already costs a second and three coins; it now costs life too. If
  playtests show trailing karts falling further behind, the spin-out's 6%
  is the first number to change.
- Keyboard steering is all or nothing, so a lower rate and wash-out may
  feel like a broken kart instead of a worn one. The first feel test on
  keyboard and gamepad decides this before a lane is built on top of it.
- The straight-line cap is a game simplification. The F1 sources describe
  tyre loss through grip and lap time, not a lower top speed; the cap is
  here because it was asked for and a player can read it.
- The rubber band at Basic (0.92 to 1.08) is larger than most of the tyre
  loss and will hide tyres on Basic bots. Narrowing it can wait until
  people have raced it.
