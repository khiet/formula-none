# ADR 0095: Worn tyres cap speed and acceleration twice as hard

## Status

Accepted (2026-11-06), pending the maintainer's playtest. Supersedes ADR 0087's
top speed cap and wash-out loads, ADR 0080's acceleration row, and the measured
lap costs an Ace bot weighs (ADR 0087). Supersedes every level's pit call, from
ADR 0081 and ADR 0085, with a call by the cliff, and ADR 0086's 15% at the flag
with the cliff. ADR 0087's steering rate, its wash-out table and ADR 0080's
braking row stand, as do every wear rate from ADR 0085, the gauge, the BOX call
and the smoke. The numbers below come from bots racing
`crates/sim/tests/bot_pits.rs`.

## Context

Worn tyres still cost too little. On ADR 0087's factors the base cap at 30% life
is 95 km/h and at 0% 76 km/h, not enough to make skipping the stop hurt. A
boost ignored the worn cap entirely: a mini-turbo or a boost pad set the same
speed on tyres worn right out as on new ones.

Issue 318 asked to double the deficit of the top speed cap and the acceleration
at every life column, keep the steering and the wash-out, and lower the wash-out
loads so a kart worn right out still slides. On those factors alone the bots
could not get round: in a pack they wear 40 to 60% a lap, against about 22% for a
clean leader, so most of a field reached 0% within three laps, where a lap costs
over a minute, and finished past the race's end cap.

## Decision

- The four life columns stay at 100, 60, 30 and 0%, and the cliff at 30%. The
  cap and the acceleration lose twice ADR 0087's share:

  | Effect, at life | 100% | 60% | 30% | 0% | ADR 0087 |
  | --- | --- | --- | --- | --- | --- |
  | Top speed cap | 1.00 | 0.94 | 0.76 | 0.40 | 1.00 / 0.97 / 0.88 / 0.70 |
  | Acceleration | 1.00 | 0.94 | 0.76 | 0.40 | 1.00 / 0.97 / 0.88 / 0.70 |
  | Steering rate | 1.00 | 0.97 | 0.88 | 0.65 | unchanged |
  | Wash-out, most slip (rad) | 0 | 0 | 0.10 | 0.15 | unchanged |
  | Braking | 1.00 | 0.98 | 0.90 | 0.75 | unchanged |

  With no coins and no tow the base cap at 60, 30 and 0% is 102, 82 and 43
  km/h. A kart worn right out runs slower than the pit limit (54 km/h).
- Every boost (a mini-turbo, a boost pad, the boost item, the star's kick and
  the rocket start) runs at its factor of the worn cap, so worn tyres cost a
  boost as much as they cost the kart's own top speed. The rocket start always
  fires on new tyres, so it does not change.
- Worn right out, a kart with no coins is too slow to drift, tows nobody, and
  the run-off costs it nothing. The tuning asserts that held a worn kart above
  the drift entry speed, the slipstream leader speed and the off-track cap are
  gone. Walls bound every course (issue 75), so grass is no shortcut.
- The wash-out loads fall from 16 and 22 to 12 and 15 m/s^2. Full lock flat out
  loads about 28.9 at 30% and 14.0 at 0%, so a kart past the cliff slides as far
  as the table says, and one worn right out slides most of 0.15 and never all of
  it, which an assert holds.
- Every bot level calls itself in at the cliff, or when its life would wear past
  the cliff by the next lane at the rate it has worn so far, its drifts' share
  taken off if it is saving its tyres. A lap past the cliff now costs more than a
  stop. Basic's 25% and Pro's 30%-or-runs-out rules are gone, since the floor
  covers both. The Ace keeps its projection for stopping sooner, and its saving,
  now against the cliff at the flag in place of 15%, and its 10% for the last
  lap is gone, since the floor decides the last lap. The seed's few points of
  offset on the life a bot reads stay.
- `BOT_TYRE_LOSS_S_PER_LAP` is measured again as ADR 0087 set out.

## Consequences

- Lap costs, seconds a lap against new tyres, mean of the three courses with a
  lane:

  | Life | 60% | 40% | 30% | 20% | 10% | 0% |
  | --- | --- | --- | --- | --- | --- | --- |
  | Measured, this tune | 1.8 | 5.3 | 10.2 | 20.7 | 32.5 | 69.1 |
  | Measured, ADR 0087 | 0.3 | 4.6 | 5.0 | 8.1 | 15.0 | 17.4 |

  At 0% Bell Deer costs 96 s a lap, Goldstone 59 and Monarch 53: one lap there
  is more than the race's end cap.
- Three laps: a lone Ace bot gets round every course with a lane on the tyres it
  started on, so a managing no-stop stays viable as ADR 0083 decided. On Bell
  Deer three laps is now where a lone Ace saves its tyres to reach the flag above
  the cliff; four wants a stop.
- Five laps: a field that can stop wins sooner than one that cannot on every
  course with a lane: 255 s against 401 on Bell Deer, 254 against 351 on
  Goldstone and 269 against 390 on Monarch.
- A Basic pack on Bell Deer bunches its first stop: the fork comes early in the
  lap and the pack has worn past the cliff by the second lap's fork, so all eight
  stop on one lap in 20 of 40 races. Nobody is capped or stranded and the lane
  takes the queue; that cell's limit is half the races, every other cell's stays
  a fifth, and none comes near it.
- The 40-race Tyres On sweep holds ADR 0085's budgets on every three-lap cell of
  the courses with a lane, and grass stays at 2 or fewer in every cell. Three
  cells break the capped budget, which the maintainer takes into the playtest:
  - Goldstone, Ace, five laps: 12 of 320 karts against 5. A pack Ace stops three
    times to a clean leader's one. The wear is real and not a short stint's
    noise, so waiting longer to trust it changed nothing.
  - The oval, which has no lane, five laps: Basic 23 and Ace 59 of 320, and Ace
    short of the final lap in 13 races. Nobody can stop there, so a pack wears to
    0% in four or five laps. A Tyres On race on a course without a lane punishes
    a pack, and that is accepted.
  The 200-race sweep is left to the maintainer.
- The three Tyres On replay fixtures are re-recorded; Tyres Off fixtures hash as
  before, since new tyres multiply every speed by exactly 1.
- Reconciliation: 1% of life now moves the cap by up to 0.36 m/s past the cliff,
  from 0.18. Still invisible, so the tolerance stays at 1%.
- Nothing drawn changes: the gauge bands and the speed readout already show it.
- Lowering pack wear (ADR 0085's bump and cornering rates) is the lever for both
  the bunching and Goldstone's capped Aces, and is left for after the playtest.
