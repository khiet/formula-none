# ADR 0012: Rooms

## Status

Accepted (2026-10-18)

## Context

Friends join by a code with no account. The room must never get stuck and
stale codes must not accumulate.

## Decision

- Rooms are created over HTTP and return a four-letter code from an
  unambiguous alphabet.
- Room state machine: Open, Countdown, Racing, Results, Closing.
- Player state machine within a room: Joining, Unready, Ready, Racing,
  Finished, Disconnected.
- A Disconnected human's kart is driven by a bot until Results or until the
  same client session token reconnects.
- The creator role passes to the next player when the creator leaves.
- A room enters Closing five minutes after the last human leaves.
- No public room list, no spectators, no mid-race join. New players may join
  between races.

## Consequences

- Both state machines are tested through the fake transport, including
  creator handover and closing after five minutes empty.
- Rooms in Open state must survive a server restart (ADR 0005).

## Outcome

The race flow landed with issue 17 (2026-10-19).

- The room follows the race's phase: Countdown at Start, Racing at Go and
  Results at the finish, with every player Finished and a Disconnected
  player's seat freed then.
- From Results a player comes back with Rematch: they are Unready and the
  room, if still showing Results, is Open, so the next race starts the way
  the first did and a newcomer joins between races. Start is honoured in
  Results too once everyone left there is Ready, so a room whose finishers
  all leave never sticks.
- The karts are in room order with the bots behind the humans; the
  Countdown names the slot driving each human kart.
