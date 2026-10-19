import type { KartTransform } from "./bridge";

/**
 * Metres above a kart's centre its label floats: over the placeholder box
 * and its cowl, clear of the kart in the chase camera.
 */
const LABEL_RISE_M = 1;

/** A name floating over another kart: which kart, what it says, and where it hangs in the world. */
export interface KartLabel {
  readonly kart: number;
  readonly text: string;
  readonly anchor: readonly [number, number, number];
}

/**
 * One label per kart but the local one, named by `name` and hung over where
 * `transforms` put the kart this frame, so it follows the kart wherever the
 * View draws it. Plain text for now: the placeholder for the nickname
 * label's art.
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
