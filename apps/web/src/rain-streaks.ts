// The rain falling (ADR 0103): thin pale streaks round the camera, thrown ahead
// of it so a kart at speed drives into them. They follow the look's level, so
// they stop under a roof, and a viewer who asks for less motion gets fewer.
// Presentation only.

import type { Vec3 } from "./kart-frame";
import { createParticles, type Particles } from "./particles";

/** Streaks per second at the whole level, and the share under reduced motion. */
const STREAKS_PER_S = 260;
const STREAKS_REDUCED_SHARE = 0.3;
const STREAK_LIFETIME_S = 0.6;
/** Every streak of its life at the whole level, with room over. */
const STREAK_CAPACITY = 180;
/** A streak is this many seconds of its fall long. */
const STREAK_STRETCH_S = 0.04;
const STREAK_FALL_M_PER_S = 24;
const STREAK_SIZE_M = 0.014;
const STREAK_COLOUR = 0xdfe7ef;
/**
 * Where a streak is born from the camera: ahead, to either side and above. The
 * ahead is past what the camera covers in the streak's life, so none falls by
 * the lens, where one would read as a pole.
 */
const STREAK_AHEAD_MIN_M = 7;
const STREAK_AHEAD_MAX_M = 38;
const STREAK_SIDE_M = 18;
const STREAK_ABOVE_MIN_M = 6;
const STREAK_ABOVE_MAX_M = 13;
/** The most emission a frame can owe, so a stalled tab does not throw a sheet of it. */
const MAX_OWED_S = 0.1;

/** The pool the streaks are thrown into. */
export function createRainStreakParticles(): Particles {
  return createParticles({ capacity: STREAK_CAPACITY, stretchSeconds: STREAK_STRETCH_S });
}

export interface RainStreaks {
  /**
   * Throw this frame's streaks at `level`, 0 to 1, round the camera at `at`
   * facing `forward`, a level unit vector, and moving at `speed` metres per second.
   */
  emit(
    at: readonly number[],
    forward: readonly number[],
    speed: number,
    level: number,
    dtSeconds: number,
  ): void;
}

/** Throws streaks into `particles`. `random` is Math.random's shape. Nothing is allocated per frame. */
export function createRainStreaks(
  particles: Particles,
  { reducedMotion }: { readonly reducedMotion: boolean },
  random: () => number = Math.random,
): RainStreaks {
  const share = reducedMotion ? STREAKS_REDUCED_SHARE : 1;
  let owed = 0;
  const position: Vec3 = [0, 0, 0];
  const birth = {
    position,
    velocity: [0, -STREAK_FALL_M_PER_S, 0] as Readonly<Vec3>,
    colour: STREAK_COLOUR,
    lifetimeSeconds: STREAK_LIFETIME_S,
    size: STREAK_SIZE_M,
  };
  const between = (least: number, most: number) => least + random() * (most - least);
  return {
    emit([x = 0, y = 0, z = 0], [fx = 0, , fz = 0], speed, level, dtSeconds) {
      if (level <= 0) {
        owed = 0;
        return;
      }
      const rate = STREAKS_PER_S * share * level;
      owed = Math.min(owed + rate * dtSeconds, Math.max(rate * MAX_OWED_S, 1));
      for (; owed >= 1; owed -= 1) {
        const covered = speed * STREAK_LIFETIME_S;
        const ahead = covered + between(STREAK_AHEAD_MIN_M, STREAK_AHEAD_MAX_M);
        const across = between(-STREAK_SIDE_M, STREAK_SIDE_M);
        // Across is the forward turned a quarter about the vertical.
        position[0] = x + fx * ahead - fz * across;
        position[1] = y + between(STREAK_ABOVE_MIN_M, STREAK_ABOVE_MAX_M);
        position[2] = z + fz * ahead + fx * across;
        particles.emit(birth);
      }
    },
  };
}
