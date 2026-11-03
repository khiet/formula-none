# ADR 0078: The creator picks the bot level; Basic is today's bots

## Status

Accepted (2026-11-03). Extends ADR 0002's bots and ADR 0061's pick on the
Title Scene; the bot constants it varies stay in the tuning module (ADR
0014).

## Context

Every room raced the same bots. They were tuned for a first race: they lift
for corners, wait a while before using an item, and the rubber band slows
them when they get ahead. A room of friends who know the courses laps them
by the second race and has nothing left to beat. Issue 270 lets the
creator pick how hard the bots drive, from three levels.

## Decision

- The level is a `bot level`: `basic`, `pro` and `ace` in code and on the
  wire, "Basic", "Pro" and "Ace" on screen. Ace above Pro is the racing
  sense: a pro drives for a living, an ace wins.
- Basic is today's bots, unchanged to the tick. Every constant Basic reads
  is today's value by name, so the replay fixtures under `assets/replays`
  pass unchanged and a retune of a bot constant still moves Basic.
- The constants that vary by level live in one `BotProfile` per level in
  `crates/sim/src/tuning.rs`, indexed by the level: the corner speed
  fraction (0.85, 0.92, 1.0), the rubber band's lower bound (0.92, 0.96,
  1.0), its upper bound (1.08 on every level), the item use delay (2 s,
  1 s, 0 s) and the rocket start chance (0.5, 0.8, 1.0). Everything else a
  bot reads (lookahead, line, kerb and wheel avoidance, pass gap, stall
  recovery) stays one constant: the levels differ in how hard a bot
  drives, not in whether it can get round. The band's upper bound stays
  put so the slipstream's assert (ADR 0077) holds whatever the level;
  Ace's lower bound of one is the lever: an Ace bot ahead of every human
  is never slowed.
- The state carries the level; the state hash does not, since it is fixed
  for the race and not part of what a tick changes. The rocket start draw
  is made at every level, so the RNG runs on the same from one seed
  whatever the level.
- The prediction carries it too (ADR 0009): a finished kart drives on
  under its bot in the prediction as on the server, so the two corner at
  the same level and the server never moves the finished kart.
- The creator picks on the Title Scene, under the flags, as the course is
  picked (ADR 0061): a radio group named `bots`, one pick for Create room
  and Solo race, remembered in the browser beside the course pick, Basic
  when missing or unknown. A player who joins by code races the room's
  level. `?bots=` overrides the pick for a solo race beside `?track=`.
- The level is fixed for the room, set at the create and shown read-only
  on the Room Scene under the course card. A different level is a new
  room, as a different course is. `POST /api/rooms` takes `botLevel`
  beside `track`; no field is Basic; an unknown level is a 400 and no
  room opens. The wire `Room` and `Countdown` carry it, so a replay
  recorded from the client reruns the server's race. `PROTOCOL_VERSION`
  goes to 21. A kept room without the field comes back at Basic (ADR 0053).
- A replay carries it as `level`; a replay without one is Basic. One
  fixture, the oval at Ace with a human idle on the grid, pins Ace's bots.
- The level changes how the bots drive, not how many there are: the count
  is still the free slots on the grid (ADR 0002).

## Consequences

- A client of version 20 is refused with the version mismatch, as every
  protocol bump is (ADR 0013).
- A bot's level is not shown on its plate or in the standings; the Room
  Scene's line is the only place a joiner reads it.
- Changing the level between rematches is a later decision; it would need
  a creator-only message and a room-wide notice.
- Retuning Pro or Ace is a change to their profile and a seed sweep; it
  leaves Basic and the fixtures alone.
