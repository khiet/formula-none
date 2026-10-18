# Assets

Blender sources and exported glTF (ADR 0007). Placeholder primitives until
milestone M1 passes its feel gate; Kenney CC0 kits afterwards. Everything in
here is CC0.

`tracks/` holds the two tracks: the oval test fixture and greybox Brands
Patch. Each is exported as a glTF for the View and a sidecar for the sim
carrying the centreline, checkpoints, item box and boost pad positions, the
spawn grid and the tagged collision meshes (asphalt, sand, grass, wall). The
sim reads the sidecar, never the mesh. `tracks/README.md` has the export
command and the scene conventions.
