# ADR 0054: Every course races three laps

## Status

Accepted (2026-10-30). Supersedes ADR 0047's "a race drops from three laps
to two" and restores ADR 0014's three laps; the rest of ADR 0047, the
course itself, stands.

## Context

ADR 0047 cut `LAPS_PER_RACE` to two so Suzuqa Patch's 1306 m lap would keep
the race near the length Brands Patch had. The constant is global, so the
oval's 438 m lap dropped to two with it. Two laps is an opening lap and a
final lap with nothing between: no middle lap to recover a bad start, and
no time for items and position changes to tell. Kart racers expect three,
and the release gate (issue 29) defines done as a three-lap race.

## Decision

- A race is three laps on every course: `LAPS_PER_RACE = 3`. There is still
  one lap count for every course; ADR 0023 deferred a per-course or
  creator-chosen count and it stays deferred.
- A race is about 3920 m on Suzuqa Patch and about 1310 m on the oval.
- Measured on this change, eight bots from Go to the last of them across
  the line, over seeds 0 to 199 in release:

  | Course       | Min     | Median  | Max     |
  | ------------ | ------- | ------- | ------- |
  | Oval         | 49.5 s  | 66.2 s  | 90.7 s  |
  | Suzuqa Patch | 161.4 s | 173.1 s | 194.7 s |

  The median is a number the next decision on race length can trade on,
  not a threshold.
- `RACE_END_CAP_S` is unchanged: a longer race does not spread the
  finishers at the line.

## Consequences

- Everything downstream reads the constant: the Instrument panel's flag
  row and `Lap n/3`, the bridge's `lapsPerRace` and the Results Scene.
  Nothing the game draws changes, and the style guide already drew three
  flags.
- A race on Suzuqa Patch takes about three minutes, half as long again as
  under ADR 0047. The local `race-of-eight` e2e, which laps it with an
  autopilot, has its lap poll and budget raised by the same half.
- No replay fixture reaches a finish, so their hashes hold.
- In the same sweep every kart finished, and no reading jumped to the other
  leg at Suzuqa Patch's bridge. A bot sat under its stall speed for over
  3 s in 2 of 200 races on the oval and 5 of 200 on Suzuqa Patch, one of
  them, Suzuqa Patch's seed 38, among the first 40; each such bot went on
  to finish. The one traced, seed 38, stalled on its first lap, which the
  lap count cannot reach. They are left to a bot change.
