# Tracks

Two tracks, each exported from Blender as a glTF the View draws and a
sidecar the sim reads (ADR 0007):

| Track                | Role                              | Files                                   |
| -------------------- | --------------------------------- | --------------------------------------- |
| `oval`               | test fixture for the sim's tests  | `oval/oval.glb`, `oval/oval.track.json` |
| `brands-patch`       | the race track                    | `brands-patch/brands-patch.glb`, `...track.json` |

## Export

One command, with Blender 4.2 or newer on `PATH` (`brew install --cask
blender` on macOS):

```sh
pnpm export:tracks
```

It runs `blender --background --python assets/tracks/build.py -- all`,
which builds each track's scene from its layout, writes the glTF and the
sidecar, and saves a `.blend` beside them for inspection (ignored by git).
Commit the glTF and sidecar: CI has no Blender, and the sim's tests include
the sidecars.

Every track is walled along both edges of the run-off the whole way
round: the export stands a wall on the sand's outer edge on each side,
following the centreline and its elevation, so a kart can leave the asphalt
but never the course and no corner can be cut (ADR 0022). A layout declares
no walls. The checkpoints are evenly spaced and span the run-off from wall
to wall. The layouts are checked without Blender, with the tests beside
them:

```sh
pnpm test:tracks
```

## Sources

The Blender source of a track is a Python layout, `trackgen/layouts/<name>.py`,
so it diffs and reviews like code. A layout lays the centreline turtle-style
from straights and arcs, then hangs everything else on it by arc length:
asphalt and run-off widths, tyre walls (`TyreWall(s, offset)`, metres
left of the centreline), elevation (hills and the jump ramp),
checkpoint count, item box rows and boost pads, and its decor:
which segments get sponsor hoardings and grandstands, and whether the
start line gets its gantry. The spawn grid is always eight slots behind
the start line. `trackgen/` is pure Python so a
layout can be checked without Blender:

```sh
python3 -c "import sys; sys.path.insert(0, 'assets/tracks'); \
  from trackgen.layouts import load; print(load('oval').path.length)"
```

`build.py` turns a layout into a Blender scene; `export.py` turns any scene
that follows the conventions below into the two files. A track modelled by
hand in Blender goes through the same exporter:

```sh
blender my-track.blend --background --python assets/tracks/export.py -- my-track assets/tracks/my-track
```

## Scene conventions

- Collection `Track`: mesh objects with a `surface` custom property, one of
  `asphalt`, `sand`, `grass`, `wall` or `tyres`. A wall holds a kart; a
  `tyres` mesh, a tyre wall's prism, bounces it off. These go into the
  glTF and, triangulated, into the sidecar as collision meshes. Wind faces
  so their normals point out of the surface; the physics relies on it.
  The asphalt carries a UV map, across the ribbon from its left edge and
  along it in metres of arc length, for the tyre marks to be laid in.
- Collection `Scenery`, optional: meshes drawn and never collided with,
  which go into the glTF only. A change here leaves the sidecar, and so
  the sim and the replay fixtures, untouched; `git diff` on the sidecar
  after an export is the check.
- A curve object `Centreline`: one closed POLY or BEZIER spline run in the
  direction of travel, starting at the start line. Spline progress is
  distance along it.
- Collections `Checkpoints`, `ItemBoxes`, `BoostPads`, `Kerbs`, `SpawnGrid`:
  empties ordered by name, facing along their local +Y, each written to the
  sidecar list of the same name (`kerbs` for the sausage kerbs). Checkpoints
  carry a `half_width` custom property.

Blender is Z up; the sidecar is written in the glTF frame (metres, Y up, -Z
forward, yaw in radians about +Y with zero facing -Z) so it matches the
mesh. `crates/sim/src/track.rs` documents the sidecar's fields and is the
reader to keep in step with `export.py`.

## Art rules

Flat colours in the 16-bit kart racer's key, one material per surface and
per piece of scenery, no textures: the palette is `MATERIALS` in
`build.py`. The scenery is the generator's kerbs on every corner, the
start line's checkers, a carbon plate with orange chevrons on each
boost pad and a striped yellow capsule on each sausage kerb, each sized to
the sim's rectangle, plus what a layout's decor asks for: hoardings with
the sponsors in `trackgen/sponsors.py` (invented, every one), and grandstands and the start gantry built from boxes to the
asset catalog's cards. Original names only; nothing may resemble a real
sponsor, a Nintendo character or circuit (ADR 0007), and no kart carries
a sponsor (ADR 0038).

The sand run-off slopes five centimetres down to the grass so no two
surfaces share a plane, which keeps "what is under this point"
unambiguous.
