# ADR 0103: Rain changes grip on a lap the seed picks

## Status

Accepted (2026-11-08), a design with nothing built. The comparison and its
evidence are in `docs/mockups/rain-on-a-seeded-lap.html`. Extends ADR 0083's
picks, ADR 0082's rolls from the seed and ADR 0098's presentation zones. The
build waits for the playtest ADR 0085 and ADR 0095 are pending, since rain sits
on top of that tyre tune.

## Context

Nothing changes during a race. Card X6 of
`docs/mockups/course-excitement.html` asks for rain on a lap the seed picks; pi
objected that it tangles grip with tyre life and risks unreadable failures in
an eight-kart pack. Three versions were compared: visual rain only, grip only,
and grip with a wet tyre fitted in the pit lane.

A throwaway trial of about 400 bot races scaled the grip factors mid-race, with
bots, wear and the pit call unchanged. It compared levels and is too small to
tune from:

- Grip alone sends karts to the pit lane. Over five laps on courses with a lane,
  Ace bots made 1.26 stops a kart after the rain's lap in the dry control and
  1.56 at the mild level below.
- A wet tyre would rarely be a choice. Packs already stop 1.2 times a kart over
  three laps in the dry, so most karts would collect one at a stop they were
  making anyway. The mild level costs about 2.9 s a lap on new tyres against a
  stop's 8 s, so two wet laps do not repay an extra stop. It would also do
  nothing on the oval, which has no lane, or on Tyres Off, which has no stops.
- Heavier grip loss overruns the end cap. Over five laps, karts past it were 39
  of 512 dry, 59 at the mild level and 104 at a heavy level (steering and
  acceleration x 0.80, braking x 0.65, wash-out + 0.10 rad).

## Decision

- **Grip only.** There is no wet tyre. A stop still restores life and nothing
  else (ADR 0082).
- **Weather is a pick**, Dry or Rain, chosen by the creator with the other picks
  (ADR 0083). It starts on Dry until it has been played. A kept room or replay
  with no pick is Dry and hashes as it does today.
- **The seed picks the lap**, from a hash that draws nothing from the sim's
  random stream: from lap 2 to the lap before the last, or lap 2 or 3 of a
  Sprint, which would otherwise have one lap to choose from.
- **The whole field gets wet together.** The rain begins on the first tick any
  kart starts the picked lap. Wetness climbs from 0 to 1 over 20 s and holds to
  the flag; it never dries. The sim's state holds the tick it began, and the
  snapshot carries that to the prediction. A snapshot's 100 ms on the ramp is
  0.5% of the wet, too little to feel.
- **Wetness scales one row of factors** on top of the tyre tables, in
  `crates/sim/src/tuning.rs` (ADR 0014), as a starting tune:

  | Effect, fully wet | Factor |
  | --- | --- |
  | Steering rate | x 0.92 |
  | Acceleration | x 0.92 |
  | Braking | x 0.80 |
  | Wash-out, added at any life | + 0.05 rad |

  The top speed cap, wear rates and mini-turbo charge do not change, so the speed
  readout and the tyre gauge keep meaning what they mean.
- **Bots keep today's rules.** Their steering shares the rate, so they slow for
  it, and the wear they read decides their pit call. Their table of lap costs
  stays dry.
- **The look is shared by every screen:**
  - Clouds gather over the leader's last quarter of the lap before, changing
    only the look.
  - The sky greys through the View's light scale, with no lightning or flashes.
  - The asphalt darkens with a soft sheen; kerbs, white lines and boost pads keep
    their colours.
  - Spray is a low, pale plume at each kart's rear wheels, no higher than their
    tops and about two kart lengths long, so wings, helmets and liveries stay
    visible through a pack. Item boxes, the runaway wheel, drones and worn-tyre
    smoke stay readable.
  - Streaks stop under roofs through presentation zones, and a viewer who asks
    for less motion gets fewer. Grip stays wet under a roof, since the sim never
    reads a zone.
  - A RAIN plate shows as it arrives, and a rain bed plays under the SFX toggle,
    levelled by wetness and never pitched.

## Consequences

- Rain works on every course and with Tyres Off, though the oval barely feels it:
  under 1 s a lap on new tyres in the trial.
- Pi's objection is partly answered. One row of factors is simpler than a second
  tyre, and spray has a budget, but worn tyres in the wet still stack, up to
  0.20 rad of wash-out at 0% life, under the drift's 0.30. Whether that reads as
  rain or as a broken kart on a keyboard is for a playtest.
- Five-lap races overrun the end cap more often, 59 karts of 512 against 39 in
  the trial. The build's sweep decides whether the row or `RACE_END_CAP_S` gives.
- The trial did not count missed pit lane forks, though a slide of 0.15 rad or
  more has carried bots past one (ADR 0087) and worn wet tyres reach it. It also
  did not race Tyres Off packs or the 20 s ramp. The build's sweep covers all
  three.
- An Ace bot underrates a stop in the wet, since its lap costs are dry.
- Dry races and existing replay fixtures hash as before; a new fixture covers
  rain. The pick bumps the protocol version and regenerates the types.
- The glossary and `docs/style-guide.html` gain their entries with the build, when
  there is something drawn to describe.
- Build issues follow this record: the sim and the pick; the look; then the
  playtest and retune, before Rain can become a default.
