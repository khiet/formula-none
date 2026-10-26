# ADR 0031: The speed cluster shows a gear, km/h and the tyres

## Status

Accepted (2026-10-25). Extends ADR 0023 and supersedes its decision on
where the greybox readouts sit: speed leaves the stack for the cluster and
the surface and connection line moves top-right. The bevelled gear box,
square shift lights and pixel kart icon are superseded by ADR 0037.

## Context

ADR 0023 kept speed as a greybox readout: one small line of m/s in the
driver's stack. It reads like a debug value, not a racing game. The F1
games show a gear, km/h and a battery gauge on the wheel, and a kart racer
can show the same without a new rule, since a gear is just a reading of
speed. Tyre wear is planned, and the panel had nowhere to show it.

Three clusters were compared on a mock Race View, kept as
`docs/mockups/speed-gear-tyre-variations.html`: twin pods with a
translucent backing, a rev strip with no box, and a steering-wheel screen.
The rev strip won: it speaks ADR 0023's 16-bit language and adds no box
behind the readouts.

## Decision

- The cluster sits bottom-left of the Race View, opposite the course map:
  a row of twelve shift lights in green, red and blue thirds, the gear in a
  bevelled box that matches the item slot, km/h beside it, and a top-down
  kart with four tyres over a percentage under it. The greybox line
  (surface and connection) moves top-right, clear of the course map and
  the countdown.
- Speed shows as whole km/h and stays positive when reversing.
- The gear and the revs are worked out in the client from the kart
  readout's speed and the sim's tuned top speed. Six gears split top speed
  evenly; the revs climb from 4,000 to 11,500 across each. N shows while
  the speed shows as 0 km/h and R while reversing. Over top speed, off a
  mini-turbo or a boost pad, the gear holds 6th on the limiter: every
  shift light turns blue and the gear flashes, steady under
  `prefers-reduced-motion`.
- The gearbox never enters the sim, the protocol or the replays. Its
  constants are display only, not gameplay, so they live in the client and
  not in the sim's tuning (ADR 0014). The bands are cut from the tuned top
  speed, so a retune moves them.
- The tyre gauge is a placeholder at 100% in the client until tyre wear
  exists. The four tyres share the one value and take its colour: green
  above 60%, yellow above 30%, red below.
- The speed, gear and tyre readouts keep their text under
  `data-instrument` names; the shift lights and the kart icon are
  decorative and hidden from assistive technology.

## Consequences

- Presentation only. The sim, the protocol and the bridge are unchanged
  and the replay fixtures stand.
- The e2e specs read km/h, and a reverse shows in the gear rather than a
  negative speed.
- Tyre wear itself (a sim field, wear per corner and its effect on grip)
  is a follow-up; it replaces the placeholder and may split the four
  tyres.
- Engine audio keeps its own gear model and does not follow the panel's
  gear. The steering-wheel screen is not offered as an option.
