# ADR 0035: A livery is a palette, and drivers are caricatures

## Status

Accepted (2026-10-26). Supersedes ADR 0034's "no driver resembles a real
person"; the ban on team names, crests, sponsors and race numbers stands.
The helmet colour and the `Helmet` material are superseded by ADR 0036:
drivers race bare-headed, and a livery is primary, secondary and accent
plus the key colour. ADR 0038 limits the sponsor ban to the cars and to
real sponsors: invented sponsors may stand on the scenery.

## Context

The glossary made a livery one fixed colour per kart, and the View painted
one material, `Livery`, in it. ADR 0034 made the cars chibi low-poly F1
cars that borrow a team's colours, and a team's car is three colours, not
one: Ferrari is red with black and yellow, Red Bull is navy with red and
yellow. One colour would paint every car as a plain block, and the eight
would read as a palette of paints rather than a grid of teams.

The fiction is a cartoon parody of Formula 1. Faceless drivers, or drivers
who resemble nobody, leave the parody with nothing to point at; the
recognisable thing about a team is its best-known driver as much as its
colours.

## Decision

- A livery is a palette: primary, secondary, accent and helmet, the four
  colours the car is painted in, plus a key colour for every single-colour
  use (the minimap, the Room Scene swatches, the kart labels, the Title
  Scene's runners and `liveryCss`).
- The car carries one material per palette colour, named `Primary`,
  `Secondary`, `Accent` and `Helmet`, and the View recolours each per slot.
  The helmet starts in the team's accent.
- The key colours are held to the distinctness margin `livery.test.ts`
  already keeps; the test wins over brand accuracy. The car's own colours
  stay faithful to the team.
- Eight liveries, each after one team: Rosso, Notte, Argento, Papaya,
  Verde, Cobalto, Rosa and Titanio. Slots still follow room join order.
- Each slot's driver is a friendly caricature of that team's best-known
  driver under a parody name, Louis Hamiltoon or Max Verstoppen: a hint of
  the real one, not a likeness. The driver is data beside the livery: the
  name, and the skin, hair and facial hair that swap on the one shared
  head.
- No team name, crest, sponsor or race number appears in the game. The
  source may say which team a livery is after, so the next person knows
  what to keep faithful to.

## Consequences

- `livery.ts` holds the palette and the driver per slot; anything that
  needs one colour reads the key colour.
- The parody names show nowhere yet; kart labels keep the player's
  nickname, and the driver portrait in the Room and Results Scenes is a
  later ticket that reads the driver data.
- Choosing a team in the Room Scene is a later ticket; until then a slot's
  team is fixed.
- The car fits today's collision box, so no sim, tuning, protocol or
  replay fixture changes.
