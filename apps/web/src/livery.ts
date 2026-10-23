// The eight liveries: one fixed colour per slot, so the kart in the View
// and the player in the Room Scene wear the same one. Picked by hand for
// distance between every pair, not walked around the hue wheel: at race
// distance two hues forty-five degrees apart blur together, two lightnesses
// do not.

export const LIVERIES = 8;

export interface Livery {
  /** sRGB, as a Three colour takes it. */
  readonly hex: number;
}

// Red, blue, yellow, green, orange, purple, white, black.
const PALETTE: readonly Livery[] = [
  { hex: 0xd62828 },
  { hex: 0x1f5fd1 },
  { hex: 0xf5c400 },
  { hex: 0x2a9d3a },
  { hex: 0xf77f00 },
  { hex: 0x8e3fbf },
  { hex: 0xf2f2f2 },
  { hex: 0x202020 },
];

export function livery(slot: number): Livery {
  const paint = PALETTE[slot % LIVERIES];
  if (paint === undefined) {
    throw new RangeError(`no livery for slot ${slot}`);
  }
  return paint;
}

/** The livery as CSS, for the Room Scene's swatches and the kart labels. */
export function liveryCss(slot: number): string {
  return `#${livery(slot).hex.toString(16).padStart(6, "0")}`;
}
