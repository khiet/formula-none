# ADR 0041: Track obstacles

## Status

Accepted (2026-10-27). The runaway wheel's path is superseded by ADR 0042,
its size by ADR 0044, the marshal's flag by ADR 0045.

## Context

The tracks' only obstacles were pipes: upright cylinders tagged `wall`
standing on the inside of the oval's turns and Brands Patch's hairpins. A
pipe held a kart dead, the way a wall does, which read as a snag rather
than a hazard to drive round. `docs/mockups/track-obstacles.html` specifies
three obstacles in their place: a tyre wall, a sausage kerb and a runaway
wheel. Issue 140 lands the tyre wall; the kerb and the wheel follow in
their own issues under this record.

## Decision

- Tyre walls replace the pipes, on the pipes' spots on both tracks. A
  tyre wall is a fifth tagged surface, `tyres`, on a 16-sided prism 1.5 m in
  radius and 1.2 m tall; the tyres drawn over it are scenery. It does not
  slow a kart, and from above it reads as `tyres`.
- A kart that touches a tyre wall bounces off it as off a kart standing
  still: `Handling::bumped` along the contact normal, judged by the
  velocity the handling sent the kart off with. The bump is the least
  kart bump plus `TYRE_WALL_RESTITUTION` of the closing speed, the whole
  of it to the kart, where a touch between karts gives each half; the
  kart loses `TYRE_WALL_SPEED_LOSS` of the closing speed, scaled by how
  square it hit. The bump decays as a kart bump does. A tyre wall never
  spins a kart out and raises no event. A wall still holds a kart with no
  bounce; a kart against both gets the wall's hold, then the tyre wall's
  bump with the part into the wall taken off.
- The sausage kerb is a raised kerb on the inside of the Brands Patch
  chicane's apex. A grounded kart that drives onto it is launched into a
  hop that lands straight, keeps a tuned share of its speed, and loses any
  drift under way with its charge.
- The runaway wheel rolls across the approach to Brands Patch's second
  hairpin from wall to wall on a fixed cycle, with a marshal's flag up
  before each pass, and spins out a kart it reaches. Its spin-out is by no
  one: `RaceEvent::SpunOut`'s `by` becomes optional.
- An obstacle that moves is a function of the race clock, computed the
  same way by the server, the prediction and a replay. It is never in the
  snapshot.

## Consequences

- No snapshot field and no protocol change for the tyre wall; the tyre
  walls are in the static track the prediction steps on, so a bounce is
  predicted. `Surface` gains `Tyres`, code 5, and the generated
  TypeScript type gains `"tyres"`.
- The tyre walls change collisions on both tracks, so the replay
  fixtures were re-recorded.
- A bot that clips a tyre wall now bounces off and drives on, where a
  pipe held it; bots lap the oval faster than the tests' scripted line.
- The kerb, the wheel and their tests, sidecar lists and asset cards come
  with their issues. Sounds for the obstacles, the wheel on the course
  map, and more kerbs or wheels on other corners are out of scope.
