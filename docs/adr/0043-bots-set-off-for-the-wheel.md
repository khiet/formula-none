# ADR 0043: Bots set off between the wheel's passes and clear the kerb early

## Status

Accepted (2026-10-28). Supersedes ADR 0042 on how a bot judges a pass.

## Context

Under ADR 0042 a bot waits for the runaway wheel when any speed from
holding its own to flat out would meet the pass. Holding its own is
standing still for a bot that has stopped, and a kart standing short of the
crossing is in the way of every pass at some moment, so a stopped bot never
judges itself clear. Its brake, held to wait, backs it up from a
standstill until the next pass is out of reach, and then it drives up and
waits again. Bots shuffled behind the crossing and the pack bunched.

The bunched pack reaches the chicane together. A bot passing a kart on the
kerb's side runs wide of its line, and it aimed for the line clear of the
kerb only from a lookahead short of the kerb, too late to come back before
pursuit cut it across the apex. Bots touched the kerb in 163 of the 200
seeds, and in 16 of 40 before the wheel bounced.

Ten of the 200 missed the race's end cap: the leader finished and the
cap ran out before every bot did.

## Decision

- A bot judges a pass from anywhere between the further of holding its
  speed and setting off from a standstill at `BOT_WHEEL_SET_OFF_FRACTION`
  of flat-out acceleration, and flat out. A moving bot is judged as
  before over the next few seconds; a stopped one sets off when the gap
  between passes lets it.
- A bot aims for the line clear of a sausage kerb from
  `BOT_KERB_APPROACH_M` short of the kerb's near end, twice its lookahead.
  The kerb's line outranks a pass, so a bot gives up passing that far out.

## Consequences

- In a sweep of 200 seeds, eight bots racing three laps of Brands Patch:
  no bot is hit by the wheel, none misses the end cap, and bots touch the
  kerb in 2, one of them in the first 40, so the kerb test races the
  shared seed again.
- The outcome is chaotic in the tuning: a fraction of 0.3 to 0.7, or an
  approach of 1.5 or 2.5 lookaheads, each leaves a seed or two in 200
  where a bot is hit or misses the end cap, from traffic rather than from
  the judgement. The wheel tests race seed 5, which missed the end cap
  before, and a lone bot standing short of the crossing at Go.
- The set-off floor also lifts a bot crawling in traffic over a long
  horizon; the sweep shows no hit from it.
- Bot behaviour away from Brands Patch's kerb and crossing is unchanged,
  so the replay fixtures are too.
