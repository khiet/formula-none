# ADR 0032: The race ends when every human has finished

## Status

Accepted (2026-10-26). Supersedes ADR 0014's initial rule that the race
ends 20 s after the first finisher.

## Context

The race ended 20 s after the first kart over the line, bot or human. A
winner kept full control after the line while the Instrument panel still
read "Lap 3/3", so they drove on into a lap they took for their last, and
the Results Scene cut in partway round it, on the very next tick. The
oval's lap is 15 to 18 s, so a winner could cross the line again before
the race ended. Twenty seconds also cut off friends a lap behind, which is
not how Mario Kart treats humans.

## Decision

- A finished kart, past the line on its last lap, is the bot's from that
  tick: its human's inputs are ignored, it keeps colliding, and it picks
  up and fires no items, so it cannot change the result for the karts
  still racing. A human coming back does not take it back.
- The race ends once no kart holds it open. A kart holds it open while it
  is a human's, has not finished and its human drives it. A kart handed to
  a bot because its player disconnected or went silent does not, so a
  race every human has left ends at once. Bots never hold a race open;
  with no humans at all, as in the sim's tests, every kart still racing
  does.
- A cap of `RACE_END_CAP_S` (60 s) starts at the first human's finish.
  When it runs out the race ends whoever is still racing.
- Karts still racing at the end are placed by progress, as before; the
  standings at that tick are the results and the outro does not move
  them.
- When the race ends, `Phase::Outro` runs for `RACE_OUTRO_S` (3 s): a bot
  drives every kart, no items are fired or taken and no laps are counted.
  Then the phase is Finished. The server sends the results at Finished,
  so a room's clients open the Results Scene after the outro, as a solo
  race does.
- A finished player sees a FINISH banner with their place and the
  finishers so far in place of the lap readout. The camera stays on their
  own kart.

## Consequences

- A slow friend is never cut off by a bot's finish, and is cut off by a
  human's only after a minute.
- An idle human holds the race open until they finish or go silent, so
  the server's tests drive their humans to the line.
- The prediction runs the same rule: the predicted kart is the bot's once
  it finishes. That bot sees no other kart, so its corrections after the
  line are not counted in the corrections per second.
- Spectating another racer after finishing and showing the cap's
  countdown are left for later.
