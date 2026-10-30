# ADR 0053: Open rooms are kept on a volume

## Status

Accepted (2026-10-30)

## Context

ADR 0005 and ADR 0012 require rooms in Open state to survive a server
restart, and ADR 0015 allows a race under way to be lost. Every deploy
restarts the server (ADR 0016), so without this a deploy during an evening
empties every lobby and the friends swap a new code. The server runs as one
container with a read-only filesystem and no database (ADR 0015, ADR 0017),
and the session token of ADR 0012 already lets a client take a seat back
over a new connection.

## Decision

- An Open room is kept as one JSON file per code under a directory the
  server is given in `ROOMS_DIR`: the room's code, track, state, every
  seat with its player and session token, and the join counter the creator
  handover orders by. The file is rewritten on every change while the room
  is Open and removed when the room enters Countdown or closes, so a room
  mid-race or in Results has no file and is not brought back.
- The directory is a named Compose volume, `rooms`, the one writable path
  in the server's container. Nothing else is persisted: a race under way,
  the metrics and the probe windows start afresh.
- The next server reads the directory before it listens. Each room comes
  back with its seats as they were, Ready included, and holds each seat
  for its player's rejoin for a tuned grace (`RESTORED_SEAT_GRACE_S`,
  thirty seconds); a seat not reclaimed by then frees as a leave would,
  and the creator role passes as usual. A room nobody returns to closes
  five minutes after the grace, as any empty room does, and its file goes
  with it.
- The client keeps the Room Scene's connection the way it keeps the race's
  (ADR 0012): on silence or a closed connection it rejoins with the session
  token until it is back or refused, the status line reading "Connection
  lost: reconnecting" meanwhile. A refusal, as when the room was mid-race at
  the restart, goes back to the Title with a message.
- The server stops promptly on SIGTERM as before, so the restart stays
  inside Docker's ten-second grace; nothing is written at shutdown, since
  the files are already current.

## Consequences

- A deploy under an open lobby is invisible beyond a moment's status line;
  a deploy under a race still ends it.
- A session token on disk is a secret for as long as the room lives; the
  volume is on the instance's root disk, reachable only through SSM, and
  the file is removed with the room.
- A player who closes the tab while the server is down holds a seat for
  the grace, during which the room cannot start; thirty seconds was chosen
  against the retry interval and a restart of under ten seconds.
- A room whose track the new server does not know is dropped and logged,
  so a track rename between deploys costs that lobby and nothing else.
- Tested through the fake transport (`crates/server/tests/restart.rs`):
  two Ready players rejoin a restarted server's room and start the race; a
  room mid-race is lost; an unclaimed seat frees after the grace; a closed
  room stays gone.
