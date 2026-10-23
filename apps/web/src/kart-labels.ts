import type { KartTransform } from "./bridge";

/**
 * Metres above a kart's centre its label floats: clear of the driver's
 * helmet, which stands about a metre over the centre, so the name never
 * covers the face.
 */
const LABEL_RISE_M = 1.5;

/** A name floating over another kart: which kart, what it says, and where it hangs in the world. */
export interface KartLabel {
  readonly kart: number;
  readonly text: string;
  readonly anchor: readonly [number, number, number];
}

/**
 * One label per kart but the local one, named by `name` and hung over where
 * `transforms` put the kart this frame, so it follows the kart wherever the
 * View draws it.
 */
export function kartLabels(
  transforms: readonly KartTransform[],
  localKart: number,
  name: (kart: number) => string,
): KartLabel[] {
  return transforms.flatMap((transform, kart) => {
    if (kart === localKart) return [];
    const [x, y, z] = transform.position;
    return [{ kart, text: name(kart), anchor: [x, y + LABEL_RISE_M, z] as const }];
  });
}
