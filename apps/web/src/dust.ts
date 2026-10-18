import type { KartTransform, Surface } from "./bridge";

/** The dust tell behind a kart: shown while it crosses the run-off, coloured by the surface. */
export interface DustTell {
  /** A Three.js colour hex. */
  readonly colour: number;
}

/** Below this the kart is not kicking anything up. */
const DUST_SPEED_M_PER_S = 1;

/** Dust colour by the run-off surface that throws it. */
const DUST_COLOURS: Partial<Record<Surface, number>> = {
  sand: 0xd8c08a,
  grass: 0x7a9a4a,
};

/**
 * The Feedback for leaving the track (GLOSSARY): a placeholder dust puff
 * behind the kart while it moves over sand or grass, in that surface's
 * colour, and nothing on the asphalt, on a wall top, in the air over
 * nothing, or at a standstill. Presentation only; reads state.
 */
export function dustTell(kart: KartTransform): DustTell | null {
  const colour = DUST_COLOURS[kart.surface];
  if (colour === undefined || Math.abs(kart.speed) < DUST_SPEED_M_PER_S) {
    return null;
  }
  return { colour };
}
