# ADR 0060: Goldstone joins the courses

## Status

Accepted (2026-10-31).

## Context

Issue 210 lets the creator pick the course from three: Bell Deer, Goldstone
and Monarch. Goldstone, after Silverstone, lands first because it uses only
pieces that already exist, so the picker (issue 213) can be proved against
two courses without new tuning. The design is
`docs/mockups/silverstone-course.html`: the map, every placement, the
decisions and a layout draft that builds and closes as written.

## Decision

- Goldstone is a flat, open and fast course run clockwise from the grid on
  the Hamilton Straight through Abbey, The Loop, the Wellington Straight,
  Luffield, Copse, the Maggotts and Becketts esses, the Hangar Straight,
  Stowe and Club. Its lap is 1323 m, within 20 m of Bell Deer's, so it races
  three laps like every course (ADR 0054). Its id is `goldstone`; the layout
  is `assets/tracks/trackgen/layouts/goldstone.py`, the mockup's draft.
- 12 m of asphalt, 6 m of run-off, 12 item boxes, 2 boost pads, 5 sausage
  kerbs, 3 tyre walls, 15 checkpoints, 8 grid slots and one wheel crossing
  on the run to Vale, all where the mockup placed them.
- No elevation, since the real circuit is an old airfield, and so no ramp:
  the course is about speed through the esses.
- No new tuning. Everything on it already exists on Bell Deer.
- The mockup's tyre pickups became coins under ADR 0056's rules: ten on the
  centreline, off the grid and down the Hangar Straight past its boost pad,
  and three out wide on the run to Vale, short of the wheel's crossing.
  Thirteen in all. The draft's wide line on the Wellington Straight moved
  because that straight holds a box row.
- The scenery is the mockup's third decision. A row of five airfield
  hangars stands outside the Hangar Straight, a new prop kind, `hangars`:
  barrel-roofed sheds 18 m wide and 24 m deep, 10 m to the ridge, each with
  a carbon door in the gable toward the track, built from the palette's
  stand, beam and carbon tones. The pits are drawn long and white as a nod
  to the Wing: 120 m long against Bell Deer's 90 m, with wall-white walls
  over beam shade under a beam-top parapet.
- The pits stand outside the Hamilton Straight, not inside it as the draft
  had them. The infield between Club and Abbey is too tight for a building
  that clears both by the 8 m rule (ADR 0050), even at the draft's 70 m.
  The grandstands that the draft put outside the straight move inside it,
  so they do not stand between the grid and the doors. A pits or hangars
  prop now faces the track from whichever side its spot is on.
- The server and the client know the course (`crates/server/src/tracks.rs`
  and `apps/web/src/track.ts`), so `?track=goldstone` races it solo. No
  room can pick it until the picker lands.

## Consequences

- Eight bots racing three laps on Goldstone took a median 154.7 s to
  finish, measured over 40 seeds: a median of 141.0 s for the winner and
  162.1 s for the last. Bell Deer measured the same way takes 162.0 s.
- No bot is stuck in 200 races of eight bots on Goldstone, so The Loop's
  12 m right-left after Village needs no change for bots.
- The replay set now runs on every track, and its coverage test asks for
  every track rather than both. `goldstone-lap-with-bots` is one kart
  following the line for a lap among seven bots.
- Bell Deer's export is unchanged: its pits keep their stand colours and
  stay inside its straight.
