# ADR 0062: The Room Scene shows the room's course card

## Status

Accepted (2026-10-31). Supersedes the "read-only badge" clause of ADR 0061;
the rest of ADR 0061 stands.

## Context

ADR 0061 put the room's course on the Room Scene as a badge: its flag and
name beside the player count. Only the Title Scene showed a course's card
(the map with its lapping dot, the blurb and the four numbers), and only for
the local pick. A player who joins by code never saw the course they were
about to race. Issue 220 asks for the card on the Room Scene.

## Decision

- The Room Scene shows the card of `room.track`, the same card the Title
  Scene draws (`createCoursePreview`), in place of the badge. Every player
  sees it, the creator included. It is read-only: the course stays fixed
  for the room, and a different course is still a new room.
- The card sits in its own column beside the header, the grid and the
  buttons. On a window taller than it is wide it stacks under the grid,
  between the grid and the status line.
- It behaves as the Title's: the lap dot laps every eight seconds and
  holds still under reduced motion, and the four numbers are measured from
  the sidecar's centreline.
- The client loads every course's centreline once at boot, for the Title
  Scene; the Room Scene reuses that list and fetches nothing.
- A room on a track no course is, the oval a test races, shows no card.
- The card is drawn a size down on the Room Scene, and the grid's columns
  narrow, so both fit side by side down to 1024x640.

## Consequences

- No protocol change: the wire `Room` already carries `track`.
- Adding a course still needs only its server entry and its client
  registry entry; the Room Scene picks the card up from the same list.
