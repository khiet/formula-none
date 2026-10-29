# ADR 0045: A marshal waves hazard flags at the wheel crossing

## Status

Accepted (2026-10-29). Supersedes ADR 0041 on the marshal's flag.

## Context

ADR 0041 has the runaway wheel roll "with a marshal's flag up before each
pass". The View drew it as a plain yellow flag standing on the marshal
post's box, `SIGNAL_YELLOW.mid` over the yellow run-off at 1.1:1, so from
the chase camera it read as scenery and no one stood at the post. The
design spec, `docs/mockups/runaway-wheel-danger-spec.html` (its Marshal
post section), puts a marshal on the post and gives him two hazard flags
and a beacon.

## Decision

- A marshal stands on the post: Oyaji, adapted from the author's own
  Purrjump character, built in code from flat-shaded primitives because he
  moves. Chibi, about 1.75 m standing. The code calls him `marshal`.
- His pose follows the wheel's kind. Quiet, he sits on a folding stool with
  a cup of tea, calm, his flags furled against the stool. Warning, he is
  on his feet in a panic, arms up, waving a flag in each hand out of phase,
  14 rad/s through 34 degrees either side of 18 degrees out. Rolling, he
  does the same and hops up to 0.18 m.
- The flags are hazard flags, not racing flags: 0.9 x 0.62 m on a 1 m
  pole, `SIGNAL_YELLOW.mid` with black diagonals 0.09 m wide every 0.2 m,
  rippling as they fly. They replace the single yellow flag.
- An amber beacon stands on the post's track-side corner, dark while the
  wheel is quiet and lit, glowing and sweeping a beam at 11 rad/s while
  the flag is up.
- All of it is Presentation, keyed on the `Wheel` the bridge reports; the
  sim and the protocol do not change.

## Consequences

- A racer sees from the approach that the pass is coming: a figure jumping
  up and two striped flags against the sky, and a lit beacon, where there
  was a yellow rectangle on yellow sand.
- The palette gains the marshal's colours, the flags' stripe and the
  beacon's three tones; the style guide copies them and repo-checks holds
  the copy to the code.
- Oyaji appears only at the marshal post. More marshals, on other corners
  or the oval, are separate decisions.
