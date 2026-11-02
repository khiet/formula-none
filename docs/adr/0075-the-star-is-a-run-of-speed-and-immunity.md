# ADR 0075: The star is a run of speed and immunity

## Status

Accepted (2026-11-02). Extends ADR 0021's contact rule and ADR 0025's
own-kart audio rule. Supersedes one line of ADR 0024, that the item slot's
characters have no idle animation, for this item alone: the landed mirror
ball shines. The icon was picked in `docs/mockups/star-icon-alternatives.html`
and the sound set in `docs/mockups/star-and-horn-sounds.html`; the full
tables and the synthesis are in `docs/mockups/star-and-air-horn.html`.

## Context

The item table had a boost, a hazard and a homing projectile. Nothing gave a
kart at the back of the field a way back into a race, and no item changed
how a kart drove for longer than a boost. Issue 263 adds the star: six
seconds of speed and immunity that spins out every kart it touches, the
comeback item, most likely at places six to eight and never at the front
two.

Three existing rules would have got it wrong. A boost replaces the boost
under way, so a run built on `Handling::boost` would end at the first
mini-turbo or pad, and a boost forces the speed every tick whatever the
brake says, which would make a hairpin undriveable. Under ADR 0021 the kart
that drives into another loses speed and bounces, and a spun-out kart stands
locked, so a star kart could not pass through what it hits. And ADR 0025 has
the audio read the own kart's readings alone, so the star music of another
kart could not be heard.

## Decision

- The sim keeps the run on its own timer, `star_left_s`, in the state hash
  and read as `star_s` on the kart readout and the snapshot. It sits outside
  what a respawn clears, so a respawn does not stop it; the finish ends it,
  and so does the race's end. A second star puts the timer back to the start
  without adding to it.
- The run multiplies the kart's top speed cap by `STAR_TOP_SPEED_FACTOR`,
  on top of its coins and its rubber band, so braking still works. The fire
  also boosts the kart for `STAR_KICK_S` at the boost item's factor, the
  jump felt on firing. A boost pad, a mini-turbo or a boost item during the
  run sets its own speed and the kart settles back to the star's cap; none
  multiplies with it. The off-track cap does not apply during a run, which
  opens no shortcut since every course is walled along both run-off edges
  (issue 75).
- Contact, extending ADR 0021: for the run the star kart's collider is in a
  collision group of its own that ordinary karts filter out and that filters
  them out, so no contact pair and no bump exist for the prediction and the
  server to disagree over. Two star karts stay in each other's filter and
  bump as usual. A hit is tested with `reaches` against the star kart's
  centre, `STAR_HIT_RADIUS_M` from a kart's box: that kart spins out, loses
  the spin-out's coins and is reported `SpunOut { by: Some(star) }`; a kart
  already spun out is passed by. The spin-out ends a hold on a hazard, which
  drops where it is; a trailed hazard shields nobody from a star.
- Immunity: a dropped hazard a star kart reaches is used up to no effect, its
  own included once it is clear of it; a held one it passes. A projectile
  that reaches a star kart is used up to no effect and looks for no new
  target; a hazard the star kart trails is not used up by it. The runaway
  wheel passes through a star kart and rolls on.
- The roll weights gain the star's column, taken from the other items'
  shares: places 1-2 roll none, 3-5 five percent, 6-8 fifteen. Each band
  still sums to the whole roll, which a `const` assert now holds.
- Bots fire a star as they fire a boost, on the first straight, and do not
  hold it for the wheel. On a run a bot neither waits for the wheel nor
  steers round the karts in its way.
- Prediction: the own client predicts the speed and the pass-through from
  the fire, and the stand-ins for the other karts take their groups from the
  snapshots' `star_s`, so it passes through a star kart as the server has
  it. Only the server spins a kart out; clients learn of it from the
  snapshot. A rewind restores `star_s` with the kart record, and the next
  tick sets the collider's groups from it.
- Presentation: the shine is worked out from `star_s`, so every client shows
  the kart in the same phase and the sim stays free of presentation. Every
  material of the kart and the driver, wheels and hair included, is pulled
  70% toward the colour of the moment and lit by it as emissive, through a
  0.5 s loop of six stops, fading to nothing over the last second; an
  additive halo and four-point sparkles go with it, and a burst of them
  marks a hit. `dressKart` therefore gives each kart its own copy of every
  material. The slot shows a faceted mirror ball on a short chain, with no
  face, whose tiles run the loop once it has landed: the exception to ADR
  0024's no-idle-animation line. A rainbow ring round the slot counts the
  own run down; the star kart's plate underline and map marker run the loop.
  Under reduced motion nothing loops: a steady white shine, the tiles
  holding their colours, no sparkles, and the ring still counting down.
- Audio, extending ADR 0025: `star-fire` plays in place of `item-fire`,
  since `soundCues` now picks the fire's cue from the item that left the
  slot. The own run's `star-music` starts with the fire and the race music
  fades to silent over about 0.2 s through a level of its own, between its
  source and its toggle's gain, keeping its place; at the end `star-end`
  plays and the race music fades back. At the finish the music fades with no
  `star-end`. A rise in `star_s` of more than 0.5 s counts as a second star
  and restarts the music; smaller rises are reconciliation and leave the
  source alone. A reconnect joins the music `STAR_S - star_s` seconds in,
  with no fire sound. It all sits under the SFX toggle, so with SFX off the
  race music stays up. The sound reading now carries the nearest other star
  kart as well: only its music plays, joined at its point in the run, none
  during the own run, heard by distance, full within 10 m and a straight
  line to silent at 60 m through a 1200 Hz lowpass with no panning, a rule
  the Air Horn will reuse. Other karts' fire and end do not play.
- The protocol's `Item` gains `Star` and `KartSnapshot` gains `star_s`, so
  the protocol version goes up; the replay fixtures are re-recorded, since
  the hash gains the timer.

## Consequences

- A kart at the back can take 225 m back from the field in one run and
  cost each kart it reaches about 68 m and three coins; the leaders never
  roll one.
- The star kart's contact with ordinary karts is no longer a bump, so a run
  that ends with two karts overlapping resolves as a bump from the next
  tick; nothing else of ADR 0021 changes.
- The snapshot grows by a byte a kart, five for one on a run (ADR 0010's
  Outcome records the new size).
- A hazard dropped by a star's hit lies where it trailed for the next kart;
  a star kart already on it uses it up as it does any dropped hazard.
- Other karts' sounds are still the star music alone; their engines, fire
  and end stay silent, and stereo panning is out of scope.
- A mirror ball mesh on the road is out of scope: the kart shines and stays
  a kart, so the ball appears only in the slot, as the boost's bottle does.
- The music's length is the run's: a retune of `STAR_S` rebuilds
  `star-music` with `pnpm export:audio`.
- `star-fire` is rendered at 0.7 of the spec's level: summed with the
  music, the race music still fading and the engine, the spec's level
  reached about 1.05 in the fire's first 24 ms, past where Web Audio clips;
  at 0.7 the worst case is about 0.98.
