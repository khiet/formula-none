# ADR 0014: Gameplay tuning module

## Status

Accepted (2026-10-18)

## Context

Tuning the feel is the main work of milestone M1 and continues after.
Constants scattered through the sim would make every tuning pass an
architecture change.

## Decision

- Gameplay, item, bot and network constants live in one module,
  `sim::tuning`, and nowhere else.
- Initial values come from the PRD: top speed 30 m/s, 0 to top in 2.5 s,
  steer rate 2.2 rad/s falling to 1.2 rad/s at top speed, hop 0.3 s, drift
  entry above 40 percent of top speed, mini-turbo blue at 0.8 s (0.6 s
  boost) and orange at 1.6 s (1.0 s boost) at +30 percent, boost pad 1.0 s
  at +40 percent, start boost window 0.3 s, sand and grass at 55 percent,
  respawn after 1.5 s out of world. Three laps, eight karts, race ends 20 s
  after the first finisher. Item roll weights by position, box respawn 3 s,
  boost item 1.5 s at +40 percent, hazard spin-out 1 s and despawn 30 s,
  homing projectile at 1.6 times top speed. Bot lookahead 12 m, item use
  within 2 s, rubber-band 92 to 108 percent of top speed.

## Consequences

- A tuning change is a one-file diff reviewed on its own.
- Tests that depend on a tuned value read it from the module rather than
  repeating the number.
- Kart stats per character are deferred; all karts read the same constants.

## M1 feel gate

Issue 10 carries the gate: "drift feels good" by hand on the two reference
machines, Chrome and Firefox on each. The values the gate is judged against
are the ones in `sim::tuning` at the commit that records the verdict; the
drift values added for it are the PRD's hop, entry speed, charge and boost
numbers plus three the PRD left to feel: a neutral drift turns as hard as
full lock (`DRIFT_TURN_NEUTRAL` 1.0), inward steer tightens and
counter-steer widens it by half of full lock (`DRIFT_TURN_STEER_RANGE`
0.5), and the nose leads the direction of travel by 0.3 rad
(`DRIFT_SLIP_ANGLE_RAD`).

Verdicts, one line per machine and browser, filled in by the developer:

- Developer's Mac, Chrome: pending
- Developer's Mac, Firefox: pending
- Mid-range Windows laptop, Chrome: pending
- Mid-range Windows laptop, Firefox: pending
