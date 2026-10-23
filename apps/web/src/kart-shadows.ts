// Where each kart's shadow disc sits: on the surface straight under the
// kart, however high the kart is. The disc is what gives a hop its height
// on screen, since the chase camera follows the kart's height only loosely.
// Presentation only, decided from the transforms the bridge reports and a
// probe of the View's track mesh; the sim and bridge are not consulted.

import type { KartTransform } from "./bridge";
import { rotate, type Vec3 } from "./kart-frame";

/** A point on a surface and the surface's unit normal there, in world metres. */
export interface SurfaceHit {
  readonly point: readonly [number, number, number];
  readonly normal: readonly [number, number, number];
}

/** Finds the surface under a point; the View implements it against the track mesh. */
export interface SurfaceProbe {
  /** The nearest surface straight below `[x, y, z]`, or undefined with nothing below. */
  under(x: number, y: number, z: number): SurfaceHit | undefined;
}

/** One kart's disc: where it lies and which way the kart faces along it. */
export interface ShadowDisc {
  /** False for a kart with no surface below, or a slot the sim did not report. */
  readonly visible: boolean;
  readonly position: Readonly<Vec3>;
  readonly normal: Readonly<Vec3>;
  /** The kart's heading, unit length, in world metres; the disc is aligned to it. */
  readonly forward: Readonly<Vec3>;
}

export interface KartShadows {
  /** One disc per slot, rewritten by `update`. */
  readonly discs: readonly ShadowDisc[];
  /** Place every slot's disc under its transform; slots past `transforms` are hidden. */
  update(transforms: readonly KartTransform[]): void;
}

interface MutableDisc {
  visible: boolean;
  position: Vec3;
  normal: Vec3;
  forward: Vec3;
}

/**
 * A disc per slot, placed each frame on whatever `probe` finds below the
 * kart's centre: remote karts get one too, since their interpolated height
 * rises on a hop like the local kart's. The discs are reused across frames,
 * so an update allocates nothing.
 */
export function createKartShadows(kartCount: number, probe: SurfaceProbe): KartShadows {
  const discs: MutableDisc[] = Array.from({ length: kartCount }, () => ({
    visible: false,
    position: [0, 0, 0],
    normal: [0, 1, 0],
    forward: [0, 0, -1],
  }));
  return {
    discs,
    update(transforms) {
      discs.forEach((disc, slot) => {
        const kart = transforms[slot];
        const hit = kart === undefined ? undefined : probe.under(...kart.position);
        disc.visible = kart !== undefined && hit !== undefined;
        if (kart === undefined || hit === undefined) {
          return;
        }
        disc.position[0] = hit.point[0];
        disc.position[1] = hit.point[1];
        disc.position[2] = hit.point[2];
        disc.normal[0] = hit.normal[0];
        disc.normal[1] = hit.normal[1];
        disc.normal[2] = hit.normal[2];
        // A kart with no yaw faces -Z.
        rotate(kart.rotation, 0, 0, -1, disc.forward);
      });
    },
  };
}
