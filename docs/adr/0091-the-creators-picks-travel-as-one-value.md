# ADR 0091: The creator's picks travel as one value

## Status

Accepted (2026-11-06). Supersedes how ADR 0078, ADR 0083 and ADR 0084 carry
their picks: `POST /api/rooms` taking `botLevel`, `tyres` and `laps` beside
`track`, and the wire's `Room` and `Countdown` carrying them as fields of
their own. What each pick is, its default and where it is shown stand.

## Context

The bot level, the Tyres pick and the laps travel from the Title Scene to
the sim one field at a time: a load, a save and a query-string read each in
the browser, a positional each through the bridge, where the level and the
Tyres crossed as strings, a field each on the wire, in the room, in a kept
room and in a replay. Each pick touched over fifty hand-written files, and a
fourth would do it again.

## Decision

- The sim owns one value for the picks, `Picks { level, tyres, laps }`, with
  a ts-rs type. Beside it in the tuning, `PICK_CHOICES` lists what each pick
  can be, in the Title Scene's order, and `FRESH_PICKS` is what a creator
  starts from: Basic, On, three laps. `Picks::default` is the race from
  before there were picks: Basic, Off, three laps. `Picks::offered` refuses
  a value no creator can make.
- `State`, `Prediction`, a replay, the bridge's race and prediction, the
  wire's `Room` and `Countdown`, the server's room and its race take the one
  value. No pick crosses the wasm seam as a string.
- `POST /api/rooms` takes `{ track, picks }`. No `picks` is `FRESH_PICKS`; a
  pick missing from `picks` is as `Picks::default` has it; a pick no creator
  can make is a 400 and no room opens.
- A replay and a kept room write the picks as fields of their own, so the
  replay format and the fixtures' hashes do not change, and a file without a
  pick reads it as `Picks::default`. A room kept before this keeps its level
  from `bot_level`.
- The course stays its own pick: the sim takes a sidecar, not a course, and
  the course is also a route.
- The browser keeps each pick under its own key, the key it is asked for by
  in the query string: `bots`, `tyres` and `laps`, as before.

## Consequences

- A fourth pick is a field of `Picks` and `PickChoices` and its constants
  in the tuning, a row on the Title Scene and a line on the Room Scene. The
  bridge, the wire, the room and the browser's storage carry it as they are.
- The wire's bytes do not change: postcard writes a struct's fields in order
  and names none, and the picks sit where their fields did. The protocol
  version stays.
- The server logs the picks as one `picks` field.
