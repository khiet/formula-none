# ADR 0085: The first tyre retune, from bot races

## Status

Accepted (2026-11-04), pending the maintainer's playtest. Supersedes ADR 0080's
1.5% / s for drifting and 6% for a spin-out; the rest of its table and its
effects stand. Supersedes ADR 0081's Pro rule that it pits "if life would run
out", now read to the next lane and not the flag. Adds a drift floor to what
ADR 0078 varies by level. The numbers below come from
bots racing `crates/sim/tests/bot_pits.rs`; nobody has played them yet.

## Context

ADR 0080 called every rate a starting tune and named the spin-out as the first
to change. Once bots could pit, eight of them on Bell Deer lost about 60% on lap
1 against the 32 to 47% ADR 0080 measured alone, since traffic adds bumps and
spin-outs. All eight of a level then stopped at the same line in a third of the
Basic races, and a kart that was hit fell further behind, the pile-on ADR 0080
warned of.

ADR 0081 gave each level a rule in a line, which left some things to settle
when building them.

## Decision

- Drifting wears 0.8% / s and a spin-out costs 4%.
- A bot calls itself in at the lane's approach, when it has a lap or more to go,
  and the call holds to the fork. Each kart reads its life a few points off,
  up to 5 either way, from a hash of the seed and its slot.
- Pro's "would run out" means before the next lane, a lap on. Read to the flag
  it would call on lap 1 of five, and again at every lane after.
- Ace reads its wear so far over the stint, and calls when life at the flag
  would be under 15% and stopping now saves more time than staying out or
  stopping on a later lap, weighed with the model's lap costs. On the last lap
  it stops only under 10%. Pro drops its drifts under 30%, as the mockup says.
  The mockup's Ace "manages when a no-stop race works" is not built.
- A bot counts as stalled under 3.2 m/s, not 1 m/s. A kart pinned on a wall
  takes a 3 m/s kick about every 2 s, so it never stayed under 1 m/s for the
  second a stall needs, and could sit there for over 100 s.
- Goldstone's and Monarch's lanes run left of the start straight, where each
  one's pit building stands. Monarch needed no new decision: left of its straight
  is empty, and its building stands across from the stands. Its boost pad moved
  from the Noghes flick to the pit bend, past the fork. Goldstone's building moved
  20 m back along the grid so every door is on the straight.
- The sweep holds budgets, not zero. Of 1600 karts on a course, level and lap
  count, the end cap may catch 4 over three laps and 24 over five; 40 may touch
  grass; in 2 races a kart may end short of the final lap. Over five laps a level's
eight bots may all make their first stop on one lap in 1 race in 5, which Basic on
Bell Deer reaches; the stuck sweep allows 8 s where it allowed 3 s, for a bot
crawling along the sand at the wheel's crossing.

## Consequences

- On this tune Ace bots stop about once over five laps and about one race in
  three over three, as ADR 0083 expected. Five-lap winners are 10 s sooner on
  Bell Deer, 11 s on Goldstone and 30 s on Monarch than on the same course with
  no lane, against the model's 8 to 14 s. That compares a field that can stop with
one that cannot, and the winner may not have stopped, so it is not one stop against
none. Monarch's gap is wider than the model's and has no explanation yet.
- The stall speed changes bots on Tyres Off as well, and Monarch's pad and both new
  lanes change those courses, so replay fixtures with bots were re-recorded where ADR
  0083 expected Off to pass untouched. Its hash still leaves out the tyres.
- The issue asked that every bot finish. In the last sweep the end cap caught 7
  of 19,200 karts over three laps and 71 over five. `RACE_END_CAP_S` stays, as
  ADR 0083 left it, so the odd far-behind kart is placed by progress. A wider cap
  would change how long people wait at the flag, so it is left to the maintainer.
- Karts launched over Monarch's walls land on grass, up to 2.25% of them in a
  sweep. Main does this too, so it is counted here and not fixed.
- The keyboard and gamepad feel of worn tyres, and whether the pile-on is gone,
  are still unjudged. The lap costs an Ace bot weighs are the model's, from before
  this retune.
