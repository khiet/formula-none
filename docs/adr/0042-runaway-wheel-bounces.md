# ADR 0042: The runaway wheel bounces off the walls

## Status

Accepted (2026-10-28). Supersedes ADR 0041 on the runaway wheel's path.
How a bot judges a pass is superseded by ADR 0043.

## Context

ADR 0041 has the runaway wheel roll across the approach to Brands Patch's
second hairpin "from wall to wall": straight over the crossing's line and
into the far wall, in 1.6 s of a 7 s cycle. It reads as scenery, and it
is dangerous only on one line, for a moment. `docs/mockups/runaway-wheel-danger-spec.html`
specifies a redesign; its Bounce section, settled in conversation after
the review pages, replaces the path.

## Decision

- A pass rolls in on the crossing's line from the wall it starts at;
  passes still alternate walls, odd ones from the left. It crosses three
  times in straight legs: it rebounds where its tyre meets a wall, its
  centre `WHEEL_RADIUS_M` off it, twice (`WHEEL_BOUNCES`), and leaves
  through the far wall on the third.
- Each leg ends at a point drawn uniformly within `WHEEL_LANDING_SPREAD_M`
  (8 m) of the crossing's line, along the track. Nothing tells the player
  where; the wheel's heading is the read.
- The wheel rolls at a constant `WHEEL_SPEED_M_PER_S` (15 m/s), so a pass
  lasts as long as its path: 4.7 to 5.4 s on Brands Patch. The cycle
  stretches to `WHEEL_PERIOD_S` 10 s, keeping 1 s of flag and leaving at
  least 3.5 s of quiet. A track whose crossing is too wide for the longest
  pass and its flag to fit in a period does not load.
- The wheel is still a function of the race clock, now with the race seed:
  each landing comes from an integer hash of the seed, the pass and the
  leg, not the sim's item generator, so it is identical natively and in
  wasm and the item rolls for a seed are unchanged. The race keeps its seed
  and hashes it.
- A kart it reaches spins out, by no one, once a pass, as before; karts
  never deflect it. It is quiet through the countdown and the outro.
- A bot follows the real path through its lane, `BOT_WHEEL_MARGIN_S`
  either side, projecting itself anywhere from holding its speed to
  gaining speed flat out to the speed it holds for the corners ahead, the
  way it faces. It brakes to wait short of where any pass can reach, since
  the reach is now too deep to coast to a stop in; within it, it stops only
  where stopping keeps it clear, and otherwise drives on through. A bot
  stalled by a crossing does not back up while the flag is up or about to
  go up.

## Consequences

- No snapshot field and no protocol change: every client has the seed from
  the race start, so the server, the prediction and a replay draw the same
  path. The bridge's wheel record gains how far through the leg under way
  the wheel is, which the View's drawn bounce starts over from at each wall.
- `WHEEL_PASS_S` is gone; `bot::drive` takes the race seed.
- Every replay fixture was re-recorded; the Brands Patch chicane fixture
  waits on the crossing long enough for the first pass to come back off
  its first rebound and spin the kart out.
- Bots that wait now brake and bunch; in a sweep of 200 seeds no bot was
  hit, but in ten a bot jammed in hairpin two behind the others and
  missed the cap. The queue backs into the chicane, where traffic shoves a
  bot onto the kerb in 163 of the 200 (16 of 40 before), so the test that
  bots keep off the kerb races a seed where none is shoved. Better queueing
  is its own issue.
- The rest of the spec, the bigger reach, the hot wheel, the painted zone
  and the marshal, lands in its own issues under its own records.
