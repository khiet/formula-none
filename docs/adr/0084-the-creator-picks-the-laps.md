# ADR 0084: The creator picks the laps

## Status

Accepted (2026-11-04). Supersedes ADR 0083's `Sprint` and `Grand Prix`: the
counts, the default and the rest of that record stand.

## Context

ADR 0083 named the race distance `Sprint` for three laps and `Grand Prix` for
five. The pick changes nothing but the lap count, so the names only stand for
a number and had to be explained beside the tiles. `Grand Prix` also already
names the poster style (ADR 0033).

## Decision

- The pick is called Laps and its choices are 3 and 5, on the Title Scene and
  in the code, the wire, `POST /api/rooms`, a replay and `?laps=`.
- Only those counts can be picked: `LAP_CHOICES` in `tuning.rs` is the list,
  and `POST /api/rooms`, the bridge and a replay refuse any other.
- The Title Scene's Laps pick has no line describing it, since the number says
  what it does.
- The Room Scene has no line for it, since the course card's Laps number
  already shows the room's pick.

## Consequences

- If a race distance ever changes more than the laps, it will need a name again
  and a record that says so.
- A kept room or a replay without a count still runs three laps.
