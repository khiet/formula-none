# ADR 0056: Coins replace the tyre pickup; the rule stands

## Status

Accepted (2026-10-30). Supersedes ADR 0055's choice of object, the tyre,
and its consequence that the tyre gauge reads the count. ADR 0055's rule
stands: placements, hitbox, the ten carried, 1% each, three lost to a
spin-out and 3 s gone. ADR 0031's tyre gauge returns to its placeholder.

## Context

A tyre lying on the asphalt read as a tyre wall's loose stack, and a tyre
gauge reading a count as a percentage read as wear the sim does not have.
Issue 200 compared five objects for the same rule in
`docs/mockups/top-speed-pickups.html` and chose the first, the coin: every
player knows a coin is picked up to go faster and lost when hit, the loss
can be shown as a spray, and the mesh is cheap.

## Decision

- The pickup is a coin at every layer: the glossary, the sim's count,
  events and tuning, the sidecar's `coins` list from the layouts' `Coins`
  collection, the protocol, the bridge, the server and the web app. Tyre
  walls, tyre marks, tyre wear and the kart's wheels keep the word tyre.
- The View draws each coin as a flat-shaded octagonal prism about 0.8 m
  across in poster gold `#ffd23f`, its rim and an F embossed on each face
  in `#b8860b`, its centre 0.6 m over the asphalt. It turns once a second
  on its vertical axis and bobs 5 cm, both off the View's clock, so every
  coin on the track turns in phase.
- The Instrument panel reads the own kart's coins as a coin and `x07 / 10`
  in the Kenney pixel font, over the shift lights. The tyre gauge holds at
  100% again, as ADR 0031 placed it.
- A spin-out's loss is shown, not simulated: as a kart's count falls, the
  View throws that many small coins out of it for 0.6 s. They are
  Presentation only and cannot be picked up; the sim has already made the
  lost coins vanish. The count only falls on a spin-out, so the View reads
  the loss off the kart readouts rather than the event, as every other
  Feedback (ADR 0030).
- `coin-pickup.wav`, a bright two-note chime, plays as the own kart's
  count rises, replacing the tyre pickup. `coin-loss.wav`, a short falling
  jingle, plays after the hit as the count falls.

## Consequences

- The protocol version is 14. The wire carries the same fields under new
  names, so the replay fixtures pass without re-recording.
- `TYRE_SET` becomes `COIN_CAP`: a kart carries a full purse, not a set.
- Scattered or collectible lost coins, bots steering for coins, coins on
  the course map and any retune stay out of scope.
