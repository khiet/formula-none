# ADR 0097: Pickups and pads set up the next fight

## Status

Accepted (2026-11-07). Supersedes ADR 0060's five sausage kerbs and its
three coins out wide on the run to Vale, and ADR 0086's lone Ace bot
that saves its way through five laps of Goldstone without a stop. Also supersedes the places that
ADRs 0047, 0060 and 0064 took from their mockups for the rows, pads and
coin lines this ADR moves. Every count those ADRs give still stands except
Goldstone's kerbs.

## Context

Every course put its rewards on the default line and its item rows wherever
they fit. Goldstone ran 937 m, 71% of the lap, from its `copse-exit` row to
its `wellington` row, and its other two rows were 106 m apart. Bell Deer's
back-straight row stood 15 m short of the straight's end, so the 2.5 s roll
(`ITEM_ROLL_S`) landed after the chicane. Centred pads on the Hangar
Straight and in Monarch's tunnel left no choice of line. Becketts' three
sausage kerbs cancelled every drift through the esses. The picks are cards
G1, G2, G3, M1, B1, B4 and X1 of `docs/mockups/course-excitement.html`.

## Decision

- A row is placed by travel time: a bot that rolls from it at racing speed
  holds the item before the next corner. Bot traces over four seeds at
  Basic and Ace measure the time; metres alone do not.
- **Goldstone's third row** moves from the National Straight to the Stowe
  exit, 15 m into `vale-approach` (1109 m). The card asked for 25 m past the
  wheel's crossing, but no spot past the hazard zone lands a roll before
  Vale: from the first clear spot an Ace bot reaches Vale in 1.5 s. On the
  Stowe exit the slowest landing is 2.8 s from the row, and the row stands
  more than 20 m short of the hazard zone. Bots waiting for the wheel stop
  no further back than 1135 m, which is clear of the row. The gaps between
  rows read 386, 432 and 505 m, against 280, 106 and 937 m before.
- **Goldstone's Hangar Straight pad** moves 3 m right, covering the right
  half of the road. The left half stays clear for the tow and the braking
  line into Stowe, a right-hander.
- **Becketts keeps only its first kerb**, inside `becketts-right`. Dropping
  `becketts-left` and `becketts` drifted bots the most of every subset tried:
  from no drift through the esses to about 1 s a lap, with about 0.9
  mini-turbos released a lap at both levels. Three single coins, 4 m out on
  the outside of each Becketts exit, mark the line that links the drifts.
  They replace the three coins on the run to Vale, since that straight now
  holds a row, so Goldstone keeps 13 coins.
- A single coin may mark a corner. A line of more than one coin still runs
  along a straight, and no coin line shares a segment with a box row.
- **Monarch's tunnel pad** moves 3 m left, covering the left half: the
  outside of the tunnel's bend and the inside of the chicane's left-hander,
  so the boost costs the chicane's entry. Driven on each side, a boosted kart
  left the most road short of the wall on the left (3.7 m from the
  centreline at worst, against 4.9 m on the right). The right half is the
  tow and the safe line in.
- **Bell Deer's back-straight row** moves to 80 m along the straight
  (997 m), not the card's 60 m. A kart boosted off the ramp is airborne
  until about 982 m, and one with a full purse lands by 991 m. The row is
  clear of the bridge (ADR 0047), and the slowest bot that rolls from it
  holds its item 3.5 s before 130R.
- **Bell Deer's Spoon coins** move from 4 m left to 4 m right on the link
  between the apexes: the wide line through the double apex, away from
  `spoon-2`'s tyre wall. The shorter inside line stays clear.
- No new mechanism: the walls (ADR 0022), the pads, the tow and the bots
  are unchanged.

## Consequences

- Goldstone's last third, from Becketts to the line, has item play, and a
  bot's Vale-row item is in hand through Vale and Club.
- On the Hangar Straight and in Monarch's tunnel the kart behind chooses
  between the pad and the tow.
- The drifts cost tyre life. Over seeds 0 to 9, a lone Ace bot on five laps
  of Goldstone with Tyres On now stops on every seed; before, one seed
  skipped its stop, its tyres down to 23% at their lowest. Saving still drops drifts
  (ADR 0086), and a lone Ace bot still runs three laps of every lane course
  without a stop. Dropping only `becketts-left` would have kept the five-lap
  no-stop, but it added a fifth of a second of drift a lap and almost no
  mini-turbos.
- The replay fixtures for the three courses are re-recorded, since boxes,
  pads, kerbs and coins moved.
- A scoped check of eight races of eight bots per course at Basic and Ace,
  old placements against new: no bot failed to finish or stuck for 3 s on
  either. Time bots spent held for the wheel fell on every course
  (Goldstone 149 to 125 s at Basic, 165 to 145 s at Ace), and the longest
  queue stayed within one kart of before. Bots drift about 1 s through
  Becketts each pass, against none. Goldstone's mean finish is 1.6 s slower
  at Basic and 4.7 s slower at Ace; Bell Deer and Monarch are within 2 s.
