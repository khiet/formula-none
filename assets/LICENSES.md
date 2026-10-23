# Asset licences

Everything under `assets/` is CC0 1.0 (ADR 0007): the Kenney kits vendored
here and the files this repo makes from them, from its own layouts and from
its own synthesis.

| Files | Source | Licence |
| --- | --- | --- |
| `karts/kenney/kart-oobi.glb`, `karts/kenney/Textures/colormap.png` | [Kenney Car Kit 3.1](https://kenney.nl/assets/car-kit), downloaded 2026-10-23 | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/), `karts/kenney/License.txt` |
| `tracks/kenney/grandStand.glb`, `tracks/kenney/overhead.glb` | [Kenney Racing Kit 2.0](https://kenney.nl/assets/racing-kit), downloaded 2026-10-23 | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/), `tracks/kenney/License.txt` |
| `karts/kart.glb`, `tracks/*/*.glb`, `tracks/*/*.track.json`, `replays/*` | this repo, from the files above and `tracks/trackgen` | CC0 1.0 |
| `audio/*.wav` | this repo, synthesised by `audio/build.py` (ADR 0008) | CC0 1.0 |

Kenney asks for no credit; we give it anyway. Every vendored Kenney
directory is named `kenney` and carries the kit's own `License.txt`, and
`tools/repo-checks` holds this table to that.
