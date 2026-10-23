# Karts

The kart every slot drives, from Kenney's CC0 Car Kit (ADR 0007). One
command, with Blender on `PATH`:

```sh
pnpm export:karts
```

It runs `build.py`, which imports `kenney/kart-oobi.glb`, flattens the
kit's texture atlas to flat materials, puts the body's and the driver's
paint in one material named `Livery`, and writes `kart.glb`. Commit the
result: CI has no Blender, and `apps/web/src/kart-asset.test.ts` reads it.

The View (`apps/web/src/kart-model.ts`) loads `kart.glb` once, clones it
per slot with `Livery` recoloured to the slot's livery, and scales it to
fill the box the sim collides with. The wheels stay nodes of their own,
`wheel-front-left` and so on, for the tells to find.
