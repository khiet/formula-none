# ADR 0058: The boost's slot character is a podium champagne bottle

## Status

Accepted (2026-10-30). Supersedes the snail with a rocket strapped to its
shell that ADR 0024 chose as the boost character and ADR 0037 drew in the
slot. What those records decide about the slot, its states, its accessible
text and the other items' characters stands.

## Context

The boost has shown in the item slot as a snail with a rocket strapped to
its shell since ADR 0024. The joke was that the slowest thing alive gets a
rocket; the problem is that a driver glancing down mid-corner reads the
snail first, and a snail reads as slow. The one item that makes you faster
should not look like the one thing that never is.

`docs/mockups/boost-icon-alternatives.html` compares candidates drawn the
way `item-sprites.ts` draws, each large, in the slot held and firing at the
panel's size, and at 64 px, beside today's snail. It took three rounds; the
page draws the last and lists the first two, whose drawings were not kept.

- Round one, cards A to F, was six takes on one red rocket: the snail's
  rocket alone, climbing or flat out, an afterburner, a nitro bottle, a
  rocket with a grin and twin boosters. None was liked; all six were the
  same idea.
- Round two, cards G to N, tried symbols, car parts and odd objects:
  chevrons, a dash plate, a turbocharger, a backfire, a lightning bolt, a
  firework, a rocket kart and a winged tyre. The chevrons were the safe
  pick but boring, and the round as a whole did not surprise.
- Round three, cards O to V, went for jokes: odd things that could shove a
  kart, a champagne bottle, a marshal's extinguisher, a chilli, a loose
  balloon, an espresso, a wind-up key, a human cannon and a hairdryer.
  Card O won: motorsport's own celebration used as a propellant, so it
  reads as winning, and its silhouette, a tilted bottle with its cork off
  the neck, holds at 64 px.

## Decision

- The boost's slot character is the podium champagne bottle, shaken: a
  green bottle tilted up and to the right, gold foil on the neck, a white
  label with a red band, the cork flying off the neck on a jet of foam.
- It has no face. The cork and the foam carry the character.
- Firing is unchanged: the plate's gold flash is the tell for every item,
  with no second frame.
- The palette gains `CHAMPAGNE_BOTTLE`, the lit, mid and shaded tones of
  `0x1f7a4a` by `tonesOf`, and `CORK`, a lit and a mid tone. The foil is
  `ROCKET_FLAME`'s tones, the label `WHITE` and `ROCKET`. The tokens only
  the snail used, `ROCKET_WINDOW`, `GREYS.white` and `GREYS.pale`, go.

## Consequences

- Presentation only. A boost never shows on the road, so there is no mesh;
  the sim, the protocol, the bridge and the glossary are unchanged, and the
  boost's strength, duration and rules are as before.
- The `ROCKET` tokens keep their names though no rocket is drawn any more:
  the runaway wheel, the drone and the bottle wear them.
- A champagne spray on the kart when the boost fires is left for later.
