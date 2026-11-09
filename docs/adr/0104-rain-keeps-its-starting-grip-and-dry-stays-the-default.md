# ADR 0104: Rain keeps its starting grip and Dry stays the default

## Status

Accepted (2026-11-09). The playtest and retune ADR 0103 left for after the
build. Changes no factor in `crates/sim/src/tuning.rs`.

## Context

ADR 0103 set one row of rain factors as a starting tune and left four
questions for a playtest: whether worn tyres in the wet read as rain or as a
broken kart on a keyboard, whose steering is all or nothing; how rain plays on
lap 3 of 3, where a stop cannot pay, and on the oval, whose 3-lap race ends
before the 20 s ramp does; whether an Ace bot's dry lap costs need a wet row;
and whether Rain can become the default pick.

A throwaway probe raced eight Ace bots on Tyres On over 40 seeds, dry against
rain on the same seeds, on every course at 3 and 5 laps, split by the lap the
seed picked: 18 to 22 races a row at 3 laps, 12 to 14 at 5. Beside it, a lone
Ace bot held at each life of `BOT_TYRE_LOSS_S_PER_LAP` raced Bell Deer,
Goldstone and Monarch without their lanes or item boxes, on seeds whose rain
began on lap 2, and timed laps 4 and 5, which run fully wet. It is too small to
tune from, and it is the evidence below.

- **Rain's cost is nearly flat across tyre life.** Against a dry lap at the same
  life, a wet lap costs about 2 to 3 s from 100% down to 30%, and under 1 s at
  20% and 10%. What a stop wins is close to the dry table: from 30% to new
  tyres, 10.8 s a lap wet against 9.6 s dry, measured; the table holds 10.2.
  At 0% a wet lap is quicker than a dry one, 56.6 s against 96.9 s on the
  mean, as ADR 0103 predicted from the slower steering loading the tyres less;
  the mean is noisy across courses.
- **Lap 3 of 3 changes nobody's race but the winner's time.** Stops made from
  the rain's lap on are 0.16 a kart wet against 0.15 dry on Bell Deer, 0.11
  against 0.12 on Goldstone and 0.26 against 0.26 on Monarch. The winner is 2
  to 3.5 s slower.
- **The oval barely gets wet.** Rain on lap 3 of 3 leaves the track 72% wet at
  the winner's flag and costs the winner about 0.6 s; on lap 2 it is fully wet
  and costs about 0.2 s.
- **Five laps add a few stops and some capped karts.** Rain adds 0.1 to 0.5
  stops a kart over five laps, most on Monarch. Goldstone sends more karts past
  the end cap in rain, 18 against 16, 19 against 8 and 13 against 6 of about
  100 karts a row; the other courses move by noise either way, and the oval's
  five laps overrun the cap in the dry as much as in rain. Karts on grass are 0
  or 1 in every row.

The keyboard playtest was not held. The maintainer chose to record the probe
now and leave the feel for a later playtest.

## Decision

- **Rain keeps ADR 0103's starting factors**: steering rate and acceleration x
  0.92, braking x 0.80, wash-out + 0.05 rad to at most 0.18 rad.
- **An Ace bot's lap costs stay dry.** Rain adds about the same to every life,
  and no stop takes it away, so the dry table weighs what a stop wins about as
  well wet as dry. There is no wet row.
- **Lap 3 of 3 and the oval stay as they are.** A last-lap scramble with no stop
  worth making, and an oval that only half gets wet, are what a seeded lap and
  a 20 s ramp give, and neither breaks a race.
- **`RACE_END_CAP_S` stays.** Goldstone's five laps in rain are recorded here and
  left alone.
- **Dry stays the default pick** until rooms have played Rain.

## Consequences

- Nothing in the sim, the protocol or the replay fixtures changes.
- Whether worn tyres in the wet read as rain or as a broken kart on a keyboard
  is still open. A playtest that finds a broken kart softens the wash-out first,
  since it is the one factor a keyboard's full lock feels, and gets its own ADR.
- Rain becoming the default is a later decision, with play in rooms as its
  evidence.
- If Goldstone's five-lap rain races start ending at the cap in play, the
  maintainer decides between the row and `RACE_END_CAP_S`, as ADR 0103 left it.
