// The pit crew's faces: seven per pit box, drawn from the head kit's pools
// by a seed of the slot, so every client draws the same crew. Like the
// drivers' names, a crew member's shows nowhere.

import {
  BROWS,
  FACIAL_HAIRS,
  type Face,
  type FacialHair,
  HAIRS,
  MOUTHS,
  NOSES,
  SKINS,
} from "./livery";

/** A crew member: a face, and their height as a share of the crew's base height. */
export interface CrewFace extends Face {
  readonly height: number;
}

export const CREW_PER_BOX = 7;
export const CREW_HEIGHT_RANGE = [0.92, 1.08] as const;

/** The hair colours a crew member is drawn in, sRGB: the drivers' and two more. */
const HAIR_COLOURS = [
  0x1a1210, 0x8a6a48, 0x5a3a22, 0x6b4528, 0x1c1c1f, 0x241a14, 0xb8955a, 0xc9c3b8, 0xa83a1e,
];
/** Most of the crew is clean-shaven. */
const FACIAL_POOL: readonly FacialHair[] = ["none", "none", ...FACIAL_HAIRS];

/** mulberry32: a small seeded uniform [0, 1) source. */
function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The crew of pit box `slot`, in member order, no hair style twice. */
export function crewRoster(slot: number): readonly CrewFace[] {
  const next = seeded(slot * 7919 + 17);
  const pick = <T>(pool: readonly T[]): T => pool[Math.floor(next() * pool.length)] as T;
  const hairs = [...HAIRS];
  const skins = Object.values(SKINS);
  return Array.from({ length: CREW_PER_BOX }, (_, member) => {
    const [hair] = hairs.splice(Math.floor(next() * hairs.length), 1);
    if (hair === undefined) throw new RangeError("more crew than hair styles");
    const [low, high] = CREW_HEIGHT_RANGE;
    return {
      skin: skins[(member + Math.floor(next() * 2) + slot) % skins.length] as number,
      hair,
      hairColour: pick(HAIR_COLOURS),
      facialHair: pick(FACIAL_POOL),
      earrings: next() < 0.15,
      brows: pick(BROWS),
      nose: pick(NOSES),
      mouth: pick(MOUTHS),
      glasses: next() < 0.15,
      height: low + next() * (high - low),
    };
  });
}
