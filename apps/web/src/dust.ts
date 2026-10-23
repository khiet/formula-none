import type { KartSize, KartTransform, Surface } from "./bridge";
import { rearWheel, rotate, toWorld, type Vec3 } from "./kart-frame";
import type { Particles } from "./particles";

/**
 * The dust tell behind a kart: shown off the rear bumper while it crosses
 * the run-off, coloured by the surface, and at the rear wheels while it
 * sits stalled at Go.
 */
export interface DustTell {
  /** A Three.js colour hex. */
  readonly colour: number;
  readonly at: "bumper" | "wheels";
}

/** Below this the kart is not kicking anything up. */
const DUST_SPEED_M_PER_S = 1;

/** Dust colour by the run-off surface that throws it. */
const DUST_COLOURS: Partial<Record<Surface, number>> = {
  sand: 0xd8c08a,
  grass: 0x7a9a4a,
};
/** The tyre smoke of a stalled kart's spinning wheels. */
const STALL_SMOKE_COLOUR = 0x9a9a9a;

/**
 * The Feedback for leaving the track and for a rocket start's stall
 * (GLOSSARY): dust puffed off the back of the kart while it moves over
 * sand or grass, in that surface's colour, and nothing on the asphalt, on
 * a wall top, in the air over nothing, or at a standstill; or smoke at the
 * rear wheels for as long as the kart is stalled at Go, wherever it stands.
 * Presentation only; reads state.
 */
export function dustTell(kart: KartTransform): DustTell | null {
  if (kart.stallSeconds !== null) {
    return { colour: STALL_SMOKE_COLOUR, at: "wheels" };
  }
  const colour = DUST_COLOURS[kart.surface];
  if (colour === undefined || Math.abs(kart.speed) < DUST_SPEED_M_PER_S) {
    return null;
  }
  return { colour, at: "bumper" };
}

export interface DustEmitter {
  /** Puff this frame's dust for `kart` into the pool, if it is on the run-off or stalled. */
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
/** The most emission a frame can owe, so a stalled tab does not empty the pool in one burst. */
const MAX_OWED_S = 0.1;

/**
 * Puffs dust off the rear bumper, or off either rear wheel, of a kart the
 * dust tell shows for, at a steady rate whatever the frame rate, in the
 * tell's colour. `random` is Math.random's shape; tests pass a fixed
 * value. Nothing is allocated per frame.
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
      // A stalled frame throws a burst, not a frame's worth of the stall.
      owed = Math.min(owed + PUFFS_PER_S * dtSeconds, PUFFS_PER_S * MAX_OWED_S);
      if (owed < 1) {
        return;
      }
      birth.colour = tell.colour;
      rotate(kart.rotation, 0, 0, 1, back);
      for (; owed >= 1; owed -= 1) {
        if (tell.at === "wheels") {
          rearWheel(kart, random() < 0.5 ? -1 : 1, size, position);
        } else {
          const across = (random() - 0.5) * PUFF_SPREAD_OF_WIDTH * size.width;
          toWorld(kart, across, -size.height / 2, size.length / 2, position);
        }
        velocity[0] = back[0] * PUFF_DRIFT_M_PER_S;
        velocity[1] = back[1] * PUFF_DRIFT_M_PER_S + PUFF_RISE_M_PER_S;
        velocity[2] = back[2] * PUFF_DRIFT_M_PER_S;
        particles.emit(birth);
      }
    },
  };
}
