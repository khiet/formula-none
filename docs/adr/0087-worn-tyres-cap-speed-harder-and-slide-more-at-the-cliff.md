# ADR 0087: Worn tyres cap speed harder and slide more at the cliff

## Status

Accepted (2026-11-05), pending the maintainer's playtest. Supersedes ADR 0080's
table of what worn tyres cost, for the top speed cap and the wash-out at the
cliff; its steering rate, acceleration and braking rows, its wear rates as ADR
0085 retuned them, and its gauge, BOX call and smoke stand. Replaces the
model's lap costs an Ace bot weighs (ADR 0085) with measured ones, which ADR 0086
deferred to this retune. This is the first result of the playtest ADR 0085 and
ADR 0086 are pending; the numbers below come from bots racing
`crates/sim/tests/bot_pits.rs`.

## Context

Worn tyres barely showed on the speedo. The base cap is 108 km/h and ADR 0080's
factor at 30% life is 0.96, so a kart at 30% life still capped at 104 km/h, and
coins, a tow and boosts multiply on top: 100 to 140 km/h was normal at the
cliff. The two bands above the cliff cost almost nothing, so a driver could not
read their wear until the gauge turned red, and skipping the stop cost too little
to discourage.

Issue 295 asked for the cap, the steering rate and the wash-out to steepen
together, with the rule that if bots at 0% could not get round, the factors
would give and not ADR 0085's sweep budgets. They could not: on the steering
column it asked for (0.94, 0.76, 0.50) and the wash-out (0.10, 0.25), whole
fields of Basic bots on Bell Deer finished without a stop, since a kart sliding
0.15 rad or more at the fork carries on past it, and even with no slide at all,
bots on the steeper steering wore to 0% and then fell 60 s behind the winner or
off Monarch's last bend. `docs/mockups/tyre-wear-and-pit-lane.html` keeps ADR
0080's table as the comparison; it is not updated.

## Decision

- The four life columns stay at 100, 60, 30 and 0%, and the cliff at 30%: the
  gauge bands, the BOX call and the worn smoke all key off 30 and 60. The cap
  steepens, the wash-out doubles at the cliff, and the steering rate gives
  back to ADR 0080's whole column:

  | Effect, at life | 100% | 60% | 30% | 0% | ADR 0080 |
  | --- | --- | --- | --- | --- | --- |
  | Top speed cap | 1.00 | 0.97 | 0.88 | 0.70 | 1.00 / 0.99 / 0.96 / 0.86 |
  | Steering rate | 1.00 | 0.97 | 0.88 | 0.65 | unchanged |
  | Wash-out, most slip (rad) | 0 | 0 | 0.10 | 0.15 | 0 / 0 / 0.05 / 0.15 |

  The base cap at 60, 30 and 0% is 105, 95 and 76 km/h. Wash-out at 0% stays
  under the drift slip angle, which the tuning asserts hold.
- The wash-out loads stay ADR 0080's 16 and 22 m/s^2. Full lock flat out on
  tyres worn right out now loads them at 20.5, under the full load, so a kart
  worn right out slides most of the table's 0.15 and never all of it, which an
  assert holds, as another holds that a hop's airtime lets the deepest slide go. Loads scaled down with the steering (11 and 15) were tried and
  put Ace bots at 0% off Monarch's last bend in a fifth of three-lap races.
- Unchanged: the acceleration and braking factors, so the first playtest has
  fewer variables; every wear rate from ADR 0085; and how coins, the tow and
  boosts multiply the worn cap. There is no separate boost cap.
- `BOT_TYRE_LOSS_S_PER_LAP` is measured, not modelled: a lone Ace bot held at
  each row's life every tick on Bell Deer, Goldstone and Monarch, raced without
  the lane and its item boxes so nothing but the tyres moves the lap, the mean of
  its second and third laps over four seeds, against the same bot on new tyres.
  Each row is the mean of the three courses. The measurement is an ignored test
  beside the sweeps, and it refuses a table a second off what it measures.
  Weighing saving's pace against a stop stays out of scope, as ADR 0086 left it.
- `State::adopt_tyre_life` is the seam that holds a kart at a life, as a
  snapshot's word would.
- ADR 0085's sweep budgets hold unchanged, and the sweep below reports two
  cells over them.
- Reconciliation: 1% of life now moves the cap by up to 0.18 m/s past the
  cliff, from 0.1; the issue reckoned 0.3 on a steeper column. Still invisible,
  so the tolerance stays at 1%.

## Consequences

- Lap costs, seconds a lap against new tyres, mean of the three courses with a
  lane:

  | Life | 60% | 40% | 30% | 20% | 10% | 0% |
  | --- | --- | --- | --- | --- | --- | --- |
  | Measured, this tune | 0.3 | 4.6 | 5.0 | 8.1 | 15.0 | 17.4 |
  | Measured, ADR 0080's factors | 0 | 3.7 | 4.1 | 5.3 | 10.3 | 13.4 |
  | The model (ADR 0080) | 0.6 | 2.0 | 2.7 | 5.0 | 7.5 | 10.6 |

  The model undercounted even ADR 0080's factors past the cliff, since it left
  out wash-out and the bot's slower corners. The courses differ: at 0% Bell Deer
  costs 14 s a lap, Goldstone 15 and Monarch 23, and at 30% they cost 3, 4 and
  8. One table serves all three, as ADR 0080 chose, so an Ace bot on Bell Deer
  overrates a stop and one on Monarch underrates it. On ADR 0080's factors Bell
  Deer at 60% ran 1.5 s a lap faster than new, the bot's line smoothed by the
  slower steering, so that row is 0 there.
- Three laps: a lone Ace bot gets round every course with a lane on the tyres
  it started on, so a managing no-stop stays viable as ADR 0083 decided. A
  pushing no-stop loses by more: a lap at 20% costs 8 s against 5 before, and
  at 0% 17 s against 13. The issue's steering column would have made that 27 s,
  and it is what put bots at 0% past the end cap.
- Five laps: a field that can stop wins sooner than one that cannot on every
  course with a lane: Ace winners over 40 races are 17 s sooner on Bell Deer,
  22 s on Goldstone and 37 s on Monarch, against ADR 0085's 10, 11 and 30.
  Over five laps a level's bots still do not all pit on one lap.
- The 200-race Tyres On sweep holds ADR 0085's budgets in 22 of its 24 cells.
  Ace bots over five laps break the capped budget on Bell Deer, 45 of 1600
  karts, and Goldstone, 36, against 24: a kart that reaches its last laps at
  0% in a pack, after a stint of 40% a lap in traffic, finishes 60 s behind a
  clean winner. Pro and Basic hold, every three-lap cell holds, and grass
  peaks at 35 on Monarch. An Ace that stops on the last lap under 20% instead
  of 10% trims those to 37 and 28, not enough, so it is not taken. What gives
  here is the maintainer's: the cap at 0%, the Ace's reading of its wear in a
  pack, or the end cap ADR 0085 left alone.
- The three Tyres On replay fixtures are re-recorded; Tyres Off fixtures hash
  as before, since their life never wears.
- Nothing drawn changes: the gauge bands and the speed readout already show it.
- The steering rate the issue wanted is left for after the playtest. Getting it
  needs bots that foresee the slide at the fork and stop before 0%, or a wider
  end cap, which ADR 0085 left to the maintainer. The feel on keyboard and
  gamepad is still unjudged; this is the first tune the maintainer plays.
