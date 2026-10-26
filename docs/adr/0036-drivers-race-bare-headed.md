# ADR 0036: Drivers race bare-headed

## Status

Accepted (2026-10-26). Supersedes ADR 0035's helmet colour, the palette's
fourth colour and the `Helmet` material, and ADR 0034's oversized helmet
and polygon helmets; the rest of both stands.

## Context

ADR 0034 gave the chibi car an oversized helmet, and ADR 0035 painted it
in the team's accent. The visor is down, so the eight drivers in
`livery.ts`, each told apart by skin, hair, facial hair and earrings, show
nowhere in the race. Mario Kart's miniature feel comes from a big bare head
on a small kart, with the character's face and hair seen from every angle.

## Decision

- The driver races bare-headed: one shared faceted head about 0.6 m
  across, with big eyes and a short jaw, facing forward, no headwear. The
  face is fixed: whites and irises in fixed colours, no blinking or eye
  movement.
- The top of the tallest hair stays at or below 1.34 m over the ground,
  where the helmet's top stood, so the chase camera and the kart labels
  keep their placing. The halo is lowered and widened to frame the face at
  chin height.
- The car carries `Skin` and `Hair` materials, which the View recolours
  from the slot's driver the way it recolours the livery. Facial hair is in
  the hair colour.
- Every hair style and facial hair in `livery.ts` is a named part of the
  car, `hair-<style>` and `facial-hair-<kind>`, beside `earrings` in a fixed
  gold; the View shows the driver's and hides the rest. Greying temples are
  a fixed grey on their hair style, so a driver has one hair colour.
- A livery is primary, secondary and accent plus the key colour. The race
  suit's shoulders stay in primary with an accent collar.

## Consequences

- The `Helmet` material and `Livery.helmet` are gone; the glossary's
  livery has three car colours.
- Adding a hair style or facial hair is a `livery.ts` value and a part in
  `assets/karts/build.py`; the kart-asset test fails until both exist.
- The driver portrait in the Room and Results Scenes can render this head;
  that is its own ticket.
- The head stays inside the height the helmet had and the collision box is
  unchanged, so no sim, tuning, protocol or replay fixture changes.
