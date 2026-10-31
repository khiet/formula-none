# ADR 0064: Monarch joins the courses

## Status

Accepted (2026-10-31).

## Context

Issue 210 lets the creator pick the course from three: Bell Deer, Goldstone
and Monarch. Goldstone landed first (ADR 0060) and the picker after it (ADR
0061). Monarch, after the Monaco street circuit, is the third and the first
with pieces no course has yet: walls 2 m off the asphalt, and a tunnel. The
design is `docs/mockups/monaco-course.html`: the map, the elevation, every
placement, the decisions and a layout draft that builds and closes as
written.

## Decision

- Monarch is a slow, walled-in course run clockwise from the grid on the pit
  straight through Sainte Devote, up Beau Rivage to Massenet and Casino
  Square, down to Mirabeau and the Grand Hotel hairpin, Portier, the
  tunnel, the chicane, along the harbour to Tabac, the two swimming pool
  chicanes, La Rascasse and Anthony Noghes. Its lap is 1301 m, within 5 m
  of Bell Deer's, so it races three laps like every course (ADR 0054). Its
  id is `monarch`; the layout is `assets/tracks/trackgen/layouts/monarch.py`,
  the mockup's draft. Its flag is Monaco's, red over white.
- 12 m of asphalt and 2 m of run-off, so the walls stand close as the
  streets' barriers do: they are what makes it Monaco. 12 item boxes, 2
  boost pads, 3 sausage kerbs, 15 coins, 15 checkpoints, 8 grid slots and
  one wheel crossing on the descent to Mirabeau, where the mockup placed
  them.
- One hill of 8 m from Sainte Devote to the tunnel mouth, 576 m long,
  cresting in Casino Square; 4.4% at its steepest. No ramp: the crest is
  the course's lift, and a ramp on a street circuit lands against a wall.
- The tunnel is new scenery, a `Tunnel(s_start, s_end)` in a layout's
  decor: about 150 m of the tunnel bend, from 10 m past its mouth to 15 m
  short of the chicane, so both portals are on the flat. A side stands
  against the back of each wall from the grass to the roof, and the roof's
  underside is 6 m over the road, clear of the chase camera's 3.2 m (which
  the track tests hold to `chase-camera.ts`). A warm lamp every 12 m along
  each side's inside, the one material that emits, so it reads lit under
  the roof. No kart meets any of it; the walls under it hold the karts.
- The mockup's tyre pickups became coins under ADR 0056's rules. Its wide
  line of three on Beau Rivage moved to the approach to Casino Square,
  still on the climb, because Beau Rivage holds a box row.
- Only one tyre wall, outside Sainte Devote. The mockup also put two inside
  the Grand Hotel hairpin, as on Bell Deer's, and its engineering notes
  asked for a check that bots do not stall on them. They do: round a 14 m
  hairpin with the walls 2 m off the asphalt, bots that met them lost
  laps there. In 40 races of eight bots, 10 karts failed to finish with the
  stacks 8 m either side of the apex, 1 with them 14 m away, and none with
  no stacks.
- A marshal post stands in the middle of a run-off too narrow for its usual
  3 m setback, so Monarch's stands inside its wall.
- The server and the client know the course (`crates/server/src/tracks.rs`
  and `apps/web/src/track.ts`), so the picker shows three flags and a room
  created on Monarch races it.
- The harbour's water and yachts and the casino building are issue 215.

## Consequences

- Eight bots racing three laps on Monarch finish in a median 173.2 s, the
  median of all 320 finishing times from 40 seeded races. The winner's
  time has a median of 154.7 s across those races, and the last kart's
  183.4 s. Bell Deer measured the same way gives 162.0 s.
- No bot is stuck in 200 races of eight bots on Monarch.
- With 2 m of run-off a kart can clear the wall at La Rascasse on a Boost;
  ADR 0063 respawns it. Three did in those 40 races.
- `monarch-lap-with-bots` puts Monarch in the replay set: one kart
  following the line for a lap among seven bots.
- No other track's export changed.
