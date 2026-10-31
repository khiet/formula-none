# ADR 0068: A kart holding an item still takes item boxes

## Status

Accepted (2026-10-31). Supersedes no ADR: none states the old rule.

## Context

A kart holding an item drove over item boxes and left them in place. A
leader holding an item could do nothing about the boxes on its line, so
they always went to the karts chasing it. Issue 247 asks for the rule
coins follow since ADR 0067: a kart in front can take boxes to deny them
to the karts behind.

## Decision

- A kart whose centre is on an item box takes it whatever its slot
  holds, and the box is gone for the tuned 3 s. With an empty slot the
  kart rolls an item as before; holding one, it keeps it and draws
  nothing from the RNG.
- The take raises the same `ItemPicked` event as any other, with its
  `item` `None` when nothing went into the slot, so the server and every
  screen hide the same box.
- A kart still takes at most one box per tick. A finished kart, any kart
  through the outro and the countdown take none; a spun-out kart still
  takes boxes.
- No cue marks a take with nothing rolled: the box disappearing is the
  only feedback. `item-pickup.wav` plays only as the own kart's slot
  fills, so the take is silent.

## Consequences

- `ItemPicked`'s `item` is optional on the wire and in the generated
  TypeScript; a client sets the local kart's slot only when it is not
  null.
- The replay fixtures whose karts drive over boxes while holding an item
  were re-recorded: those boxes are gone for the karts behind, which now
  roll from a different point in the RNG.
- A leader holding an item now takes the boxes ahead of it, so the karts
  behind roll fewer items than they did.
- A sound or Instrument panel cue for a box taken with nothing rolled,
  and bots steering to take or deny boxes, stay out of scope.
