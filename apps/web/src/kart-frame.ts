// Where things are on and around a kart, from the transform the bridge
// reports: the sim's box is the frame, Y up, the nose at -Z. Every function
// writes into a buffer the caller owns, so a frame of eight karts allocates
// nothing.

import type { KartReadout, KartSize } from "./bridge";

export type Vec3 = [number, number, number];

/** Which rear wheel: -1 the left, 1 the right, looking forward. */
export type Side = -1 | 1;

/**
 * The bottom of the kart's box under the rear wheel on `side`, in world
 * metres, written to `out`. A tyre's centre sits its radius, half the
 * kart's height, in from the box's rear and half its width in from the side.
 */
export function rearWheel(kart: KartReadout, side: Side, size: KartSize, out: Vec3): Vec3 {
  return toWorld(
    kart,
    side * (size.width / 2 - TYRE_WIDTH_M / 2),
    -size.height / 2,
    size.length / 2 - size.height / 2,
    out,
  );
}

/** How wide a tyre's print is on the road. */
export const TYRE_WIDTH_M = 0.2;

/** The point `[x, y, z]` in the kart's frame, in world metres, written to `out`. */
export function toWorld(kart: KartReadout, x: number, y: number, z: number, out: Vec3): Vec3 {
  rotate(kart.rotation, x, y, z, out);
  out[0] += kart.position[0];
  out[1] += kart.position[1];
  out[2] += kart.position[2];
  return out;
}

/** The vector `[x, y, z]` turned by the unit quaternion `[qx, qy, qz, qw]`, written to `out`. */
export function rotate(
  [qx, qy, qz, qw]: readonly [number, number, number, number],
  x: number,
  y: number,
  z: number,
  out: Vec3,
): Vec3 {
  // v' = v + 2 q.xyz x (q.xyz x v + q.w v)
  const cx = qy * z - qz * y + qw * x;
  const cy = qz * x - qx * z + qw * y;
  const cz = qx * y - qy * x + qw * z;
  out[0] = x + 2 * (qy * cz - qz * cy);
  out[1] = y + 2 * (qz * cx - qx * cz);
  out[2] = z + 2 * (qx * cy - qy * cx);
  return out;
}
