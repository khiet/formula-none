// The parts a driver wears on the one shared head (ADR 0036), by the node
// names assets/karts/build.py exports them under: the View and the poster
// kart both show the driver's and leave the rest.

import { type Driver, FACIAL_HAIRS, HAIRS } from "./livery";

/** Every swappable part on the head, by the name the exporter gives its node. */
export const DRIVER_PARTS: ReadonlySet<string> = new Set([
  ...HAIRS.map((hair) => `hair-${hair}`),
  ...FACIAL_HAIRS.filter((facialHair) => facialHair !== "none").map(
    (facialHair) => `facial-hair-${facialHair}`,
  ),
  "earrings",
]);

/** The parts `driver` wears; a clean-shaven driver's "none" matches no part, so no facial hair shows. */
export function partsOf(driver: Driver): ReadonlySet<string> {
  return new Set([
    `hair-${driver.hair}`,
    `facial-hair-${driver.facialHair}`,
    ...(driver.earrings ? ["earrings"] : []),
  ]);
}
