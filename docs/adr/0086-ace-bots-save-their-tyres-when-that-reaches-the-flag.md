# ADR 0086: Ace bots save their tyres when that reaches the flag

## Status

Accepted (2026-11-04), pending the maintainer's playtest. Builds what ADR 0085
left out, the mockup's Ace that "manages when a no-stop race works", and supersedes
ADR 0085's Ace call where saving reaches the flag: that bot stays out even if a
stop would pay. The rest of ADR 0085 stands. The numbers below come from bots racing; nobody has played them
yet, and ADR 0085's retune has not been played either.

## Context

An Ace bot already skips a stop when its tyres will last, but it drove no
differently to make them last. Dropping its drifts for a whole race, in 40 races
on each course, left a lone Ace bot 0.49 to 0.58 of its wear and a pack 0.52 to 0.78,
against the mockup's guess of a third less for 2.5% of pace. The pace was not one
number: a lone Ace bot with no drifts was 5.6% slower over three laps of Bell Deer
and 1 to 3% faster on Goldstone and Monarch.

## Decision

- At each lane approach, where it makes its pit call, an Ace bot projects its
  life to the flag at its rate of wear so far. If that ends under 15% and the
  same projection at 0.67 of the rate does not, it saves its tyres. It does not
  call for the pits, and it drops its drifts until the next approach reads
  otherwise.
- Only the projection decides. The bot does not weigh saving's pace against a
  stop's time, because that pace differs by course and the lap costs it would be
  weighed with are the model's, from before ADR 0085's retune.
- Saving means dropping drifts and nothing else: the mini-turbos go with them,
  and the bot takes the corner on full lock as Pro does on worn tyres.
- A course with no lane has no approach, so an Ace bot there never saves. There is
  no stop to skip and no call to make.

## Consequences

- Over 40 races of each course, a lone Ace bot on five laps of Goldstone now skips
  its stop in 70% of them and finishes 4.2 s sooner. On Bell Deer and Monarch it
  still stops, as the projection at 25% a lap falls short even saving.
- A pack barely changes. Stops per kart moved by 0.04 at most, too small to read
  over 40 races. The first lap's traffic takes 41 to 46%, which the rate over the
  stint reads as the bot's pace, so most karts in a pack look too far short to save. Whether to read
  wear differently is left until ADR 0085's retune has been played.
- The rate over a stint falls while a bot saves, so a later reading can say pushing
  now reaches the flag and the bot drifts again, saving on alternate laps. It saves
  no more than it must, and can land a few points short: those lone five-lap
  Goldstone bots that skip their stop reach the flag on a mean of 11%, under the 15%
  they aim for.
- Replay fixtures with Ace bots on Tyres On change; `goldstone-bots-pit` was
  re-recorded. Tyres Off hashes as before, since its life never wears.
