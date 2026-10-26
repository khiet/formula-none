# Karts

The car every slot drives: a chibi low-poly open-wheel F1 car (ADR 0034),
built from primitives by a script. One command, with Blender on `PATH`:

```sh
pnpm export:karts
```

It runs `build.py`, which builds the car in metres to the box the sim
collides with, every face flat-shaded in a flat material, and writes
`kart.glb`. Commit the result: CI has no Blender, and
`apps/web/src/kart-asset.test.ts` reads it.

Four materials are the livery (ADR 0035): `Primary`, `Secondary`, `Accent`
and `Helmet`, exported a neutral grey. The View
(`apps/web/src/kart-model.ts`) loads `kart.glb` once, clones it per slot
with each recoloured to that colour of the slot's palette, and scales it to
fill the box. The other materials (carbon, tyre, rim, visor) keep the
colours `build.py` gives them. The wheels stay nodes of their own,
`wheel-front-left` and so on, placed at their axles and unrotated, for the
tells to find.
