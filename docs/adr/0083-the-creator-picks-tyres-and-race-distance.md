# ADR 0083: The creator picks tyres and the race distance

## Status

Accepted (2026-11-03). Supersedes ADR 0054's one lap count for every race;
its three laps stay as the Sprint. Extends ADR 0061's and ADR 0078's picks
on the Title Scene. The specification and its evidence are in
`docs/mockups/tyre-wear-and-pit-lane.html`. ADR 0084 supersedes its names for
the race distance: the pick is the laps, 3 or 5.

## Context

The model behind ADR 0080 and ADR 0081 found three laps too short for a stop
to be the normal plan. A driver who manages their tyres and never stops
beats one who pushes and stops by 2.4 to 3.9 s on every course with a pit
lane. At five laps it flips: one stop beats none by 8 to 14 s. Wearing tyres
faster is not the fix, because pushing already reaches the red band on lap
2. So strategy needs a longer race, and a room that wants the race it has
today should be able to keep it.

## Decision

- The creator picks a race distance, `Sprint` for three laps or `Grand Prix`
  for five, on any course, Sprint by default. Three laps remains the normal
  short race.
- The creator picks Tyres On or Off. Off is today's race, with no wear, no
  BOX plate and no effect on handling. On is the default once bots pit and
  all three lanes are in; until then it defaults to Off, so main is always
  playable.
- Both are picked like the bot level (ADR 0078): on the Title Scene, one
  pick for Create room and Solo race, remembered in the browser, fixed for
  the room and shown read-only on the Room Scene. A different pick is a new
  room.
- `POST /api/rooms` takes both beside `track` and `botLevel`, and an unknown
  value is a 400. A kept room or a replay without a field is Sprint and Off,
  whatever the default has become, so what was raced stays what it was.
- With Tyres Off the state hash is unchanged, so existing replay fixtures
  pass untouched. `LAPS_PER_RACE` becomes the Sprint's constant and a room
  setting carries the count to everything that read it.

## Consequences

- A Grand Prix takes about 4.5 minutes, against 2.6 for a Sprint.
- A Grand Prix on the oval has no lane. At the 19.5% a lap Ace bots lose
  there, a kart that pushes reaches red during lap 4 and ends near 0%,
  still able to finish but with no way to fix it. It is left as a race of
  pure tyre management until playtests say otherwise.
- At Sprint distance a stop is a rescue, and the undercut barely exists.
- Each pick bumps the protocol version and regenerates the types.
- `RACE_END_CAP_S` stays, as ADR 0054 left it: a longer race does not
  spread the finishers at the line.
