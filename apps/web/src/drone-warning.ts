// The drone warning (ADR 0076): a homing projectile chasing the own kart
// is warned of, on the Instrument panel and by the audio, from the warning
// distance in. The sim names each projectile's target in every snapshot,
// so the warning is read off the readout alone and helps every kart,
// whatever it holds.

import type { KartReadout, Projectile } from "./bridge";
import { groundDistance } from "./kart-frame";

/** A projectile chasing the own kart is warned of from this far: about 2.2 s out at the speed it closes on a kart at top speed. */
export const DRONE_WARNING_M = 40;

/** Metres along the ground to the nearest of `projectiles` chasing `kart`, at `own`; null while none does. */
export function incomingDroneMetres(
  projectiles: readonly Projectile[],
  kart: number,
  own: Pick<KartReadout, "position">,
): number | null {
  return projectiles
    .filter((projectile) => projectile.target === kart)
    .reduce<number | null>((nearest, projectile) => {
      const metres = groundDistance({ position: projectile.position }, own);
      return nearest === null ? metres : Math.min(nearest, metres);
    }, null);
}

/** Whether the warning shows for a drone `metres` away: within the warning distance. */
export function droneWarned(metres: number | null): boolean {
  return metres !== null && metres <= DRONE_WARNING_M;
}
