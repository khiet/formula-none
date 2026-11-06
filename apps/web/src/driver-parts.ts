// The parts a face wears on the one shared head (ADR 0036), by the node
// names assets/karts/build.py exports them under: the View's karts, the
// poster kart and the pit crews all show the face's and leave the rest.

import { BROWS, FACIAL_HAIRS, type Face, HAIRS, MOUTHS, NOSES } from "./livery";

/** What a face wears over its hair: nothing, as a driver races, or the pit crew's headset. */
export type Headgear = "none" | "headset";

/** Every swappable part on the head, by the name the exporter gives its node. */
export const FACE_PARTS: ReadonlySet<string> = new Set([
  ...HAIRS.map((hair) => `hair-${hair}`),
  ...FACIAL_HAIRS.filter((facialHair) => facialHair !== "none").map(
    (facialHair) => `facial-hair-${facialHair}`,
  ),
  "earrings",
  ...BROWS.map((brows) => `brows-${brows}`),
  ...NOSES.map((nose) => `nose-${nose}`),
  ...MOUTHS.map((mouth) => `mouth-${mouth}`),
  "glasses",
  "headset",
]);

/**
 * The parts `face` wears under `headgear`; a clean-shaven face's "none"
 * matches no part, so no facial hair shows. The headset goes over the hair,
 * so the hair stays whatever it is.
 */
export function partsOf(face: Face, headgear: Headgear = "none"): ReadonlySet<string> {
  return new Set([
    `hair-${face.hair}`,
    `facial-hair-${face.facialHair}`,
    `brows-${face.brows}`,
    `nose-${face.nose}`,
    `mouth-${face.mouth}`,
    ...(face.earrings ? ["earrings"] : []),
    ...(face.glasses ? ["glasses"] : []),
    ...(headgear === "headset" ? ["headset"] : []),
  ]);
}
