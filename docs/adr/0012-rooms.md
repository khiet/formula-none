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

Disconnect and rejoin landed with issue 20 (2026-10-19).

- `Joined` deals a session token, which a `Rejoin` presents to take the
  seat back. Mid-race the kart is the player's again from the rejoin, and
  the server sends the Countdown of the race under way so a client that
  lost it can rebuild it. Any other token is refused as an unknown session;
  a rejoin while the old connection lives moves the seat to the new one and
  tells the old one it was replaced.
- The server counts a human it has heard no input from for the tuned
  silence (two seconds) as Disconnected and hands their kart to a bot, as
  it does on a dropped connection; their next input on a connection that
  never dropped hands it back. The silence is counted from the first input,
  so a client still loading the race is not taken for a lost one.
- The client shows the connection banner after the same silence from the
  server, or when its connection ends, and rejoins over a new connection
  until it is back or refused. It keeps the token in local storage, so the
  room's link reopened in the same browser rejoins the race too.
