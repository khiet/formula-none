# Asset licences

Everything under `assets/` is CC0 1.0 (ADR 0007): the Kenney kits vendored
here and the files this repo makes from them, from its own layouts, from its
own Blender scripts and from its own synthesis.

| Files | Source | Licence |
| --- | --- | --- |
| `fonts/kenney/Kenney Mini Square.ttf` | [Kenney Fonts](https://kenney.nl/assets/kenney-fonts), downloaded 2026-10-25 | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/), `fonts/kenney/License.txt` |
| `karts/kart.glb` | this repo, built from primitives by `karts/build.py` (ADR 0034) | CC0 1.0 |
| `tracks/*/*.glb`, `tracks/*/*.track.json`, `replays/*` | this repo, built from primitives by `tracks/build.py` and `tracks/trackgen` | CC0 1.0 |
| `audio/*.wav` | this repo, synthesised by `audio/build.py` (ADR 0025) | CC0 1.0 |
| `apps/web/src/item-sprites.ts` (from the repo root) | this repo, the item slot's characters drawn as string grids (ADR 0024) | CC0 1.0 |
| `apps/web/src/item-meshes.ts` `marshalMesh` (from the repo root) | this repo: Oyaji, the marshal, adapted from the author's own Purrjump character (ADR 0045); no third-party licence | CC0 1.0 |

Kenney asks for no credit; we give it anyway. Every vendored Kenney
directory is named `kenney` and carries the kit's own `License.txt`, and
`tools/repo-checks` holds this table to that.
