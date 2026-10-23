// The surface under a kart, read off the View's own copy of the track mesh
// by raycasting in Three.js. The bridge offers no surface query, and
// Presentation never asks the sim (GLOSSARY: Presentation); the View has the
// mesh anyway, so the shadow discs find the road here.

import { type Object3D, Raycaster, Vector3 } from "three";
import type { SurfaceProbe } from "./kart-shadows";

/** A probe starts its ray this far above the kart's centre, so a kart pressed into the road is still over it. */
const PROBE_FROM_ABOVE_M = 1;
/** What a probe reports for a surface whose mesh carries no normals. */
const WORLD_UP = new Vector3(0, 1, 0);

/**
 * The surface under a point, found by casting a ray straight down through
 * `track`. The ray starts a metre over the point so a kart pressed into the
 * road still finds it, but the road is never more than `reachM` above a
 * kart's centre, half the kart's height, so a hit further up is an overhang
 * and is passed over for the nearest hit beneath it.
 */
export function trackProbe(track: Object3D, reachM: number): SurfaceProbe {
  const raycaster = new Raycaster();
  const from = new Vector3();
  const down = new Vector3(0, -1, 0);
  const normal = new Vector3();
  return {
    under(x, y, z) {
      raycaster.set(from.set(x, y + PROBE_FROM_ABOVE_M, z), down);
      const hit = raycaster
        .intersectObject(track, true)
        .find((candidate) => candidate.point.y <= y + reachM);
      if (hit === undefined) {
        return undefined;
      }
      // The hit's normal is in the mesh's own frame; the point is already in the world's.
      normal
        .copy(hit.normal ?? hit.face?.normal ?? WORLD_UP)
        .transformDirection(hit.object.matrixWorld);
      return {
        point: [hit.point.x, hit.point.y, hit.point.z],
        normal: [normal.x, normal.y, normal.z],
      };
    },
  };
}
