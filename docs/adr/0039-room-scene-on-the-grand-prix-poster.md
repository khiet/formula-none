# ADR 0039: The Room Scene joins the Grand Prix poster

## Status

Accepted (2026-10-27). Extends ADR 0033: it takes up the Room Scene that
ADR 0033 left in its old look.

## Context

ADR 0033 set the Title and Results Scenes on the Grand Prix poster and left
the Room Scene a browser-default form on black. Going from the Title to the
Room, and back from the Results, jumped between two looks. Issue 136 brings
the Room Scene into design A of
`docs/mockups/title-and-results-variations.html` with no new mockup.

## Decision

- The Room Scene uses the same backdrop as the other two: the red poster,
  the slowly rotating rays, the chequered bands and the Instrument panel's
  pixel face. Its styles stay in `apps/web/index.html` under `#room-scene`,
  sharing the poster rules with `#title-scene` and `#results-scene`, and
  the layout scales with the window, clean down to 1024x640. Nothing
  pulses, and everything holds still under `prefers-reduced-motion`.
- The heading stays "Room" and the code, the code in its own span, with a
  player count beside it ("3/8").
- The link stays visible. A "Copy link" chip beside it writes the link to
  the clipboard and reads "Copied!" for about 2 s. When the clipboard is
  missing or refuses, as on plain http to a LAN address, the chip selects
  the link so it can be copied by hand.
- Only the players who have joined are drawn, in two staggered columns
  like a starting grid, filling left, right, left, right: at most 4 rows.
  Each is a flat side-on kart in their livery with their nickname. "You"
  and "Creator" are badges styled like the Fastest lap badge; the state is
  a chip, green for Ready and muted for Not ready. Joining and Disconnected
  karts are dimmed.
- The status line, with the same wording, sits above the buttons. The
  buttons are the poster's chunky ones: Start is the green primary, and is
  drawn flat and grey while disabled; Ready/Unready and Leave are yellow.

## Consequences

- Presentation only, apart from the client-only Copy link: no sim,
  protocol, server or audio change.
- `#room-scene`, `data-slot`, `data-room-code`, `data-room-link`,
  `data-room-state`, the livery's `data-livery` and the button names stay,
  and each badge keeps "(you)" and "(creator)" in its text with the
  parentheses hidden, so the e2e specs find the Scene as before.
- All three Scenes outside a race now share one look.
