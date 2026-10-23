import type { KartSize, KartTransform, Surface } from "./bridge";
import { rotate, toWorld, type Vec3 } from "./kart-frame";
import type { Particles } from "./particles";

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
 * The Feedback for leaving the track (GLOSSARY): dust puffed off the back
 * of the kart while it moves over sand or grass, in that surface's colour,
 * and nothing on the asphalt, on a wall top, in the air over nothing, or at
 * a standstill. Presentation only; reads state.
 */
export function dustTell(kart: KartTransform): DustTell | null {
  const colour = DUST_COLOURS[kart.surface];
  if (colour === undefined || Math.abs(kart.speed) < DUST_SPEED_M_PER_S) {
    return null;
  }
  return { colour };
}

export interface DustEmitter {
  /** Puff this frame's dust for `kart` into the pool, if it is on the run-off. */
  emit(kart: KartTransform, dtSeconds: number): void;
}

/** Puffs per second off a kart on the run-off. */
const PUFFS_PER_S = 30;
/** A puff drifts back off the bumper this fast and rises as it spreads. */
const PUFF_DRIFT_M_PER_S = 1;
const PUFF_RISE_M_PER_S = 0.8;
/** A puff is born anywhere across this share of the kart's width. */
const PUFF_SPREAD_OF_WIDTH = 0.8;
const PUFF_LIFETIME_S = 0.8;
const PUFF_SIZE_M = 0.3;

/**
 * Puffs dust off the rear bumper of a kart the dust tell shows for, at a
 * steady rate whatever the frame rate, in the surface's colour. `random` is
 * Math.random's shape; tests pass a fixed value. Nothing is allocated per
 * frame.
 */
export function createDustEmitter(
  particles: Particles,
  size: KartSize,
  random: () => number = Math.random,
): DustEmitter {
  /** Puffs owed and not yet thrown; a fraction carries to the next frame. */
  let owed = 0;
  const position: Vec3 = [0, 0, 0];
  const velocity: Vec3 = [0, 0, 0];
  const back: Vec3 = [0, 0, 0];
  const birth = {
    position,
    velocity,
    colour: 0,
    lifetimeSeconds: PUFF_LIFETIME_S,
    size: PUFF_SIZE_M,
  };
  return {
    emit(kart, dtSeconds) {
      const tell = dustTell(kart);
      if (tell === null) {
        return;
      }
      owed += PUFFS_PER_S * dtSeconds;
      if (owed < 1) {
        return;
      }
      birth.colour = tell.colour;
      rotate(kart.rotation, 0, 0, 1, back);
      for (; owed >= 1; owed -= 1) {
        const across = (random() - 0.5) * PUFF_SPREAD_OF_WIDTH * size.width;
        toWorld(kart, across, -size.height / 2, size.length / 2, position);
        velocity[0] = back[0] * PUFF_DRIFT_M_PER_S;
        velocity[1] = back[1] * PUFF_DRIFT_M_PER_S + PUFF_RISE_M_PER_S;
        velocity[2] = back[2] * PUFF_DRIFT_M_PER_S;
        particles.emit(birth);
      }
    },
  };
}
