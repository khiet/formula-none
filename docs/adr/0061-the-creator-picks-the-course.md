# ADR 0061: The creator picks the course; it is fixed for the room

## Status

Accepted (2026-10-31). The Room Scene's read-only badge is superseded by
ADR 0062, which shows the course card there instead.

## Context

Until now every room raced the server's default track, Bell Deer, and a
solo race took its track from `?track=` alone. Issue 210 lets the creator
pick from the courses, and Goldstone (ADR 0060) is the second one. The
design is `docs/mockups/course-picker.html`.

## Decision

- The pick is made on the Title Scene, in a second column beside the way
  in: a flag tile per course, a radio group named `course`, and a preview
  card. One pick covers Create room and Solo race. A player joining by
  code races the room's course, whatever they picked.
- `POST /api/rooms` takes an optional JSON body, `{ "track": "goldstone" }`.
  No body gives the server's default, Bell Deer. A track the server does
  not have is a 400, and no room opens.
- The course is fixed once the room is open. The wire `Room` carries it as
  `track`, so the Room Scene shows it as a read-only badge, and every
  Countdown names it as before. A different course is a new room.
  `PROTOCOL_VERSION` goes to 15.
- The last pick is saved in the browser beside the audio toggles. A
  missing or unknown pick falls back to Bell Deer.
- `?track=` still overrides the pick for a solo race, so tests and smoke
  runs can load any track, the oval included.
- The client's course registry in `apps/web/src/track.ts` holds each
  course's name, the circuit it is after, its flag and its blurb. The
  picker draws a tile per entry. The oval is a track but not a course, so
  it has no tile and no badge.
- The preview's four numbers (lap, laps, tightest bend, longest straight)
  are measured from the sidecar's centreline, not written into the
  registry, so they cannot drift from the track the sim races. The lap dot
  holds still under reduced motion.

## Consequences

- A client of version 14 is refused with the version mismatch, as every
  protocol bump is (ADR 0013).
- A kept room already carried its track (ADR 0053, ADR 0059), so a room
  created on Goldstone comes back on Goldstone after a restart.
- Adding a course is an entry in `crates/server/src/tracks.rs` and one in
  the client's registry, with its flag in `apps/web/src/flags.ts`.
