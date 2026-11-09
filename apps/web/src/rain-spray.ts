// The spray off wet tyres (ADR 0103): a low pale plume at each kart's rear
// wheels. Its budget is what keeps a pack readable: no puff rises past the top
// of the wheels and none trails more than two kart lengths, so wings, helmets,
// liveries and everything else on the road stay in sight through it.
// Presentation only, from the readouts and the wetness.

import type { KartReadout, KartSize } from "./bridge";
import { rearWheel, rotate, type Vec3 } from "./kart-frame";
import { createParticles, type Particles } from "./particles";

/** Puffs a kart throws per second, fully wet, and how long each lives. */
const SPRAY_PUFFS_PER_S = 70;
const SPRAY_LIFETIME_S = 0.4;
/** A kart's share of the pool: every puff of its life, with room over. */
const SPRAY_CAPACITY_PER_KART = 34;
/** How much of its birth size a puff gains per second. */
const SPRAY_GROWTH_PER_S = 1;
/** A puff is born this share of the kart's height across, sitting on the road. */
const SPRAY_BIRTH_SHARE = 0.4;
/**
 * A puff rises this share of the kart's height per second. With the birth size
 * and growth above, the top of a puff at its death is 78% of the way up the box,
 * under the tops of kart.glb's rear wheels at 86%: the budget.
 */
const SPRAY_RISE_SHARE_PER_S = 0.75;
/** How far behind its wheel a puff ends, in kart lengths. */
const SPRAY_TRAIL_LENGTHS = 2;
/** How fast a puff spreads off its wheel, outward. */
const SPRAY_SPREAD_M_PER_S = 0.5;
/** Below this the tyres throw nothing. */
const SPRAY_SPEED_M_PER_S = 2;
/** The most emission a frame can owe, so a stalled tab does not throw a burst. */
const MAX_OWED_S = 0.1;

/** Pale, so it never reads as the worn tyres' darker smoke (ADR 0080). */
const SPRAY_COLOUR = 0xe8eef4;

/** The pool for `kartCount` karts' plumes. */
export function createSprayParticles(kartCount: number): Particles {
  return createParticles({
    capacity: kartCount * SPRAY_CAPACITY_PER_KART,
    growth: SPRAY_GROWTH_PER_S,
  });
}

export interface RainSpray {
  /** Throw this frame's puffs for `kart` at `wetness`, if its tyres are throwing any. */
  emit(kart: KartReadout, wetness: number, dtSeconds: number): void;
}

/**
 * Throws a kart's plume into `particles`, thicker as the track wets. Each puff
 * is carried along with the kart less a closing speed that takes it the trail's
 * length over its life, so the plume is that long however fast the kart goes.
 * One emitter a kart, as each carries its own fraction of a puff. `random` is
 * Math.random's shape. Nothing is allocated per frame.
 */
export function createRainSpray(
  particles: Particles,
  size: KartSize,
  random: () => number = Math.random,
): RainSpray {
  const closing = (SPRAY_TRAIL_LENGTHS * size.length) / SPRAY_LIFETIME_S;
  const puff = SPRAY_BIRTH_SHARE * size.height;
  let owed = 0;
  const position: Vec3 = [0, 0, 0];
  const velocity: Vec3 = [0, 0, 0];
  const birth = {
    position,
    velocity,
    colour: SPRAY_COLOUR,
    lifetimeSeconds: SPRAY_LIFETIME_S,
    size: puff,
  };
  return {
    emit(kart, wetness, dtSeconds) {
      const moving = Math.abs(kart.speed) >= SPRAY_SPEED_M_PER_S;
      if (wetness <= 0 || !moving || kart.airborne || kart.surface !== "asphalt") {
        owed = 0;
        return;
      }
      const rate = SPRAY_PUFFS_PER_S * wetness;
      owed = Math.min(owed + rate * dtSeconds, Math.max(rate * MAX_OWED_S, 1));
      const carried = kart.speed - Math.sign(kart.speed) * Math.min(Math.abs(kart.speed), closing);
      for (; owed >= 1; owed -= 1) {
        const side = random() < 0.5 ? -1 : 1;
        rearWheel(kart, side, size, position);
        position[1] += puff / 2;
        // The kart faces -Z in its own frame.
        rotate(
          kart.rotation,
          side * random() * SPRAY_SPREAD_M_PER_S,
          SPRAY_RISE_SHARE_PER_S * size.height,
          -carried,
          velocity,
        );
        particles.emit(birth);
      }
    },
  };
}
