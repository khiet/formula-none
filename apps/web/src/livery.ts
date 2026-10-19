// The eight liveries: one fixed colour per slot, so the kart in the View
// and the player in the Room Scene wear the same one.

export const LIVERIES = 8;

/** A livery's hue, saturation and lightness; the hue walks the wheel once over the grid. */
export interface LiveryColour {
  /** 0..1 around the wheel. */
  readonly hue: number;
  /** 0..1. */
  readonly saturation: number;
  /** 0..1. */
  readonly lightness: number;
}

export function liveryColour(livery: number): LiveryColour {
  return { hue: (livery % LIVERIES) / LIVERIES, saturation: 0.8, lightness: 0.5 };
}

/** The livery as CSS, for the Room Scene's swatches. */
export function liveryCss(livery: number): string {
  const { hue, saturation, lightness } = liveryColour(livery);
  return `hsl(${Math.round(hue * 360)}deg ${Math.round(saturation * 100)}% ${Math.round(lightness * 100)}%)`;
}
