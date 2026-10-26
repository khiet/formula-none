# Assets

Blender sources and exported glTF (ADR 0007). Everything in here is CC0;
`LICENSES.md` says what came from where.

`karts/` holds the kart the View draws, a chibi low-poly F1 car built from
primitives with its paint in four swappable materials, the livery.
`karts/README.md` has the export command.

`tracks/` holds the two tracks: the oval test fixture and Brands Patch.
Each is exported as a glTF for the View and a sidecar for the sim carrying
the centreline, checkpoints, item box and boost pad positions, the spawn
grid and the tagged collision meshes (asphalt, sand, grass, wall). The sim
reads the sidecar, never the mesh, so the scenery in the glTF (kerbs,
hoardings, stands) never touches the rules. `tracks/README.md` has the
export command and the scene conventions.

`audio/` holds the race's sounds (ADR 0025): one WAV per SFX (hop,
mini-turbo, item pickup, item fire, hit, lap) and the music loop, all
synthesised by `audio/build.py`, so nothing in it is anyone else's work.
`pnpm export:audio` rewrites them; commit the output, since the web app
imports the files.

`fonts/` holds the Instrument panel's pixel face (ADR 0023), one TTF from
Kenney's font pack vendored under `fonts/kenney` with the pack's own
licence. The web app imports the file and registers it with the FontFace
API; nothing else in the repo reads it.
