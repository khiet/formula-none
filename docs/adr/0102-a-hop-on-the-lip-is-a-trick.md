# ADR 0102: A hop on the lip is a trick

## Status

Accepted (2026-11-08). Takes up the trick boost ADR 0101 left waiting.

## Context

Air off a ramp was a cost with no skill in it. Card B5 of
`docs/mockups/course-excitement.html` asks for a hop pressed on the lip to earn
a small boost on landing, and issue 331 for the sim to judge it, Ace bots to do
it and the View to show it.

## Decision

- **The sidecar carries each ramp's lip**, as `ramp_lips`: the middle of its top
  edge at that edge's height, the way off it and how far it reaches across. The
  sim needs no other word of the ramp; its slope stays in the asphalt's mesh
  (ADR 0101). A course without a ramp has none.
- **The window is in seconds, 0.1 either side of the lip**, at the speed the
  kart closes on it, so a boosted kart's is no shorter to press than a slow
  one's. At the pad's 42 m/s that opens about 4 m up the slope. It is judged at
  the press, from where the kart is: across the lip's reach, closing on it, and
  with its centre over the lip's height, which the slope's top half and the
  flight clear and the flat road under the high end does not.
- **A press counts when it hops or falls.** On the ground it hops as any press
  does. Past the lip the kart is falling and a press in the air does not hop
  again, so there it only counts. A press in a hop's own airtime does not, so
  mashing the button gets one try. A fresh press is needed, as for a hop, so one
  held through the lip earns nothing.
- **The landing pays**, on the first grounded tick: the orange mini-turbo's
  1.3 for 0.6 s, burning orange. A boost under way with more speed left in it
  (seconds over factor above one) stays, so a trick never costs a kart that
  fired an item on the lip; the pad's 0.2 s of 1.4 that the kart lands with
  (ADR 0101) has less, so the trick takes over from it.
- **A drift is left alone.** The press is judged beside the drift, whose charge
  neither gains nor loses for it. A spin-out or respawn in the air loses the trick.
- **Ace bots trick**, on the tick the window opens; Basic and Pro drive off. A
  bot still keeps to the flat half, so it tricks only where traffic puts it on
  the ramp's: choosing the ramp is card X5.
- **The readout carries `trick`** while one waits for the landing, and a
  snapshot carries that: the View's flourish reads it for every kart, the own
  kart's sound for its own. A prediction adopts the server's word and rewinds on
  a disagreement, since the two can judge a press at the window's edge a tick
  apart.
- **The state hash counts a trick only while one is earned**, so no fixture
  without a trick was re-recorded. `bell-deer-ramp-trick` and
  `bell-deer-ace-bots-trick` carry one through the determinism gate.
- **The View** turns the driver's head one whole turn about the neck in 0.35 s,
  by the clock, so a trick earned late still ends facing forward; none under
  reduced motion. A rising run of notes sounds at the press, and the landing's
  boost sounds as a pad's.

## Consequences

- An early press lands further off than a late one, since the hop's launch
  replaces the slope's climb: about 0.8 s of flight against 0.4 s. It still
  pays, and the extra air is the trade.
- Any ramp a later layout adds works the same with no change to the sim.
