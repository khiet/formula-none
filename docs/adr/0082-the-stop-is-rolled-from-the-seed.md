# ADR 0082: The stop is rolled from the seed

## Status

Accepted (2026-11-03). Extends ADR 0081. The specification and its evidence
are in `docs/mockups/tyre-wear-and-pit-lane.html`.

## Context

A stop's length is meant to be down to chance, as a crew's luck is in F1:
mostly clean, sometimes slow, rarely a disaster. The sim already has a
random stream for item rolls, but a draw from it here would shift every
later item roll, and a browser predicting one kart could not make the same
draw.

## Decision

- The stationary time comes from a hash of the race seed, the slot and that
  kart's stop count, the way the runaway wheel uses the seed. It draws
  nothing from the sim's random stream.
- It is pure chance. The driver only waits for green: no launch bonus and
  nothing to press. A launch bonus would turn the stop into a reaction test
  on a roll the driver is meant to accept.

  | Roll | Chance | Stationary |
  | --- | --- | --- |
  | Record | 5% | 1.9 to 2.2 s |
  | Clean | 65% | 2.3 to 3.0 s |
  | Slow | 20% | 3.1 to 4.5 s |
  | Botched | 8% | 5.0 to 7.0 s |
  | Disaster | 2% | 9.0 to 12.0 s |

  The median is 2.8 s and the mean 3.3 s. The tail stops at 12 s where F1's
  worst of 2025 reached 29.04 s, so one bad roll hurts without ending the
  race.
- A stop restores life to 100%. A kart that entered on 97% still gets the
  full stop.
- The crew and the light read the kart readout; nothing about them enters
  the sim. The readout shows a timer counting up and then the final time
  coloured by roll, and the Results Scene gains stops and best stop.

## Consequences

- Server and browser agree on the exact tick of green, with no correction.
- With eight karts stopping once, about one race in seven has a disaster
  and about half have a botched stop. The chances are judgement, not
  measured, and can be retuned in the tuning module (ADR 0014).
- The seed alone sets a kart's stop times, so anyone who knows the seed and
  the hash could work them out. The seed is not treated as secret for this.
