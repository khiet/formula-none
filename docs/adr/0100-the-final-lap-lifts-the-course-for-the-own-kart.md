# ADR 0100: The final lap lifts the course for the own kart

## Status

Accepted (2026-11-07).

## Context

The last lap looked and sounded like the first, so a race ended flat. The
course should build to a climax as the own kart starts its last lap
(`docs/mockups/course-excitement.html`, card X2): the gantry boards read
FINAL LAP, the crowd rises, fireworks go up over the course's landmark and
the music lifts.

## Decision

- The set piece runs while the own kart's standing is on the race's last
  lap, the picked laps, and the race is running: from the lap's first tick
  to the kart's finish, though the standing's lap stays on the last one
  after it. It is the client's, from the readout and the picks; nothing
  goes on the wire, and each client lifts for its own kart, not the
  leader's. The sim, the sidecar and the replays are untouched.
- The gantry: a board over the chequer on each face of the beam, carbon
  between two red bands, FINAL LAP in white in the Instrument panel's pixel
  face, unlit so it reads like a lit display. The View finds the beam by
  its chequer's nodes, so no track is re-exported, and a course without a
  gantry, the oval, shows none.
- The fireworks: a shell every 0.5 s, a third as many for a viewer asking
  for less motion, bursting 12 to 26 m over a point in the landmark's
  footprint: Bell Deer's Circuit Wheel, Goldstone's hangars and Monarch's
  harbour, found by their nodes. Each shell is 48 streaking stars in one of
  the palette's brightest colours, cooling as they burn out over 1.8 s,
  drawn solid and unlit, since against the daylight sky additive stars
  wash out to white. A course without a landmark, the oval, has none.
- The crowd: the crowd loop comes up at its whole level all round the lap,
  louder than any stand's presentation zone (ADR 0098) and wherever the
  kart is.
- The music lifts by a tempo change, not a new stem: the race music loop
  plays 8% faster, a semitone and a third up, and 1.3 times as loud, gliding in
  and back out at the finish without losing its place. No new audio file.

## Consequences

- Every course's last lap starts with FINAL LAP on the gantry, fireworks
  over its landmark, a louder crowd and quicker, louder music.
- In a room each driver's course lifts on their own last lap; a lapped
  kart sees no FINAL LAP while the leader runs it.
- The fireworks are seen only where the landmark is in view; a lap that
  never looks toward it sees few of them. Their sound is the crowd's.
- A pitch shift comes with the tempo change; a stem of its own would need
  a second loop kept in step with the first.
