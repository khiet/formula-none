// The star's shine (ADR 0075): the rainbow a kart on a run cycles through,
// worked out from the seconds the run has left, which every client reads
// from the same snapshot, so every screen shows the kart in the same phase.
// Presentation only: it reads `starS` and never the inputs. The sparkles
// thrown off the bodywork and the burst that marks a hit live here too.

import type { KartReadout, KartSize } from "./bridge";
import { rotate, type Vec3 } from "./kart-frame";
import type { Particles } from "./particles";

/**
 * The six rainbow stops, in loop order: red, orange, yellow, green, cyan,
 * violet. docs/style-guide.html copies them under the star's entry.
 */
export const STAR_RAINBOW: readonly number[] = [
  0xff4d4d, 0xff9f1c, 0xffe14d, 0x3ddc84, 0x37bedd, 0xb05cff,
];
/** Seconds one pass through the six stops takes: a 2 Hz sweep, under the three flashes a second limit. */
export const STAR_LOOP_S = 0.5;
/** How far every material is pulled toward the colour of the moment through the run. */
export const STAR_PULL = 0.7;
/** Seconds before the run's end over which the pull fades to nothing. */
export const STAR_FADE_S = 1;
/** Under reduced motion the kart holds a steady white shine, pulled this far. */
export const STAR_STILL_SHINE = 0xffffff;
export const STAR_STILL_PULL = 0.55;
/** The ball's tiles, a step apart along the loop. */
export const STAR_TILES = 6;

/** `a` mixed `t` of the way toward `b`, both Three.js colour hexes. */
export function mixColour(a: number, b: number, t: number): number {
  return [16, 8, 0].reduce((mixed, shift) => {
    const from = (a >> shift) & 0xff;
    const to = (b >> shift) & 0xff;
    return mixed | (Math.round(from + (to - from) * t) << shift);
  }, 0);
}

/**
 * The colour of the loop `starS` seconds from the end of a run: a smooth
 * blend between neighbouring stops, so the shine never steps.
 */
export function starColour(starS: number): number {
  const stops = STAR_RAINBOW.length;
  const at = ((((starS / STAR_LOOP_S) % 1) + 1) % 1) * stops;
  const from = Math.floor(at) % stops;
  const to = (from + 1) % stops;
  return mixColour(STAR_RAINBOW[from] ?? 0, STAR_RAINBOW[to] ?? 0, at - from);
}

/** How far the kart is pulled toward the shine `starS` seconds from the end: full, then fading over the last second. */
export function starPull(starS: number): number {
  return STAR_PULL * Math.min(1, Math.max(0, starS) / STAR_FADE_S);
}

/** The colour of the ball's `tile`, a step along the loop from the one before it, `seconds` into the loop. */
export function starTileColour(seconds: number, tile: number): number {
  return starColour(seconds - (tile * STAR_LOOP_S) / STAR_TILES + STAR_LOOP_S);
}

/** What the kart's shine comes to this frame: the colour and how far it is pulled toward it. */
export interface Shine {
  readonly colour: number;
  readonly pull: number;
}

/**
 * The shine for a kart with `starS` seconds of run left, or none off a
 * run. Under `reducedMotion` the shine is a steady white that neither
 * loops nor fades.
 */
export function starShine(starS: number | null, reducedMotion: boolean): Shine | null {
  if (starS === null) return null;
  if (reducedMotion) return { colour: STAR_STILL_SHINE, pull: STAR_STILL_PULL };
  return { colour: starColour(starS), pull: starPull(starS) };
}

/** Sparkles thrown off the bodywork each second through the run. */
const SPARKLES_PER_SECOND = 8;
/** A sparkle's size in metres, and how long it lives. */
const SPARKLE_SIZE_M = 0.18;
const SPARKLE_LIFE_S = 0.45;
/** A sparkle drifts back off the kart about this fast, and up a little. */
const SPARKLE_DRIFT_M_PER_S = 2;
const SPARKLE_RISE_M_PER_S = 1.2;
/** Sparkles in the burst that marks a hit, and how fast they fly out. */
export const HIT_BURST_COUNT = 24;
const HIT_BURST_SPEED_M_PER_S = 9;
const HIT_BURST_LIFE_S = 0.6;
const HIT_BURST_SIZE_M = 0.22;
/** The most emission a frame can owe, so a stalled tab does not empty the pool in one burst. */
const MAX_OWED_S = 0.25;
/** A sparkle is born white and cools to the rainbow colour of the moment. */
const SPARKLE_WHITE = 0xffffff;

export interface StarSparkles {
  /** Throw this frame's sparkles off `kart`, shining in `colour`, while it is on a run. */
  emit(kart: KartReadout, colour: number, dtSeconds: number): void;
  /** The burst that marks a hit: sparks thrown out from `position` in `colour`. */
  burst(position: readonly [number, number, number], colour: number): void;
}

/**
 * The star's sparkles for one kart: four-point glints born anywhere on the
 * bodywork, white with a rainbow tip, left behind the kart as it drives
 * on. `random` is Math.random's shape; tests pass a fixed sequence.
 * Nothing is allocated per frame.
 */
export function createStarSparkles(
  particles: Particles,
  size: KartSize,
  random: () => number = Math.random,
): StarSparkles {
  let owed = 0;
  const position: Vec3 = [0, 0, 0];
  const velocity: Vec3 = [0, 0, 0];
  const offset: Vec3 = [0, 0, 0];
  const back: Vec3 = [0, 0, 0];
  const birth = {
    position,
    velocity,
    colour: SPARKLE_WHITE,
    cooledColour: 0,
    lifetimeSeconds: SPARKLE_LIFE_S,
    size: SPARKLE_SIZE_M,
    restitution: null,
    glow: true,
  };
  return {
    emit(kart, colour, dtSeconds) {
      owed = Math.min(owed + SPARKLES_PER_SECOND * dtSeconds, SPARKLES_PER_SECOND * MAX_OWED_S);
      if (owed < 1) return;
      birth.cooledColour = colour;
      birth.lifetimeSeconds = SPARKLE_LIFE_S;
      birth.size = SPARKLE_SIZE_M;
      rotate(kart.rotation, 0, 0, 1, back);
      for (; owed >= 1; owed -= 1) {
        // Anywhere on the box the kart fills, a little above its floor.
        rotate(
          kart.rotation,
          (random() - 0.5) * size.width,
          (random() * 0.8 + 0.1) * size.height,
          (random() - 0.5) * size.length,
          offset,
        );
        position[0] = kart.position[0] + offset[0];
        position[1] = kart.position[1] + offset[1];
        position[2] = kart.position[2] + offset[2];
        velocity[0] = back[0] * SPARKLE_DRIFT_M_PER_S;
        velocity[1] = back[1] * SPARKLE_DRIFT_M_PER_S + SPARKLE_RISE_M_PER_S * random();
        velocity[2] = back[2] * SPARKLE_DRIFT_M_PER_S;
        particles.emit(birth);
      }
    },
    burst(at, colour) {
      birth.cooledColour = colour;
      birth.lifetimeSeconds = HIT_BURST_LIFE_S;
      birth.size = HIT_BURST_SIZE_M;
      position[0] = at[0];
      position[1] = at[1];
      position[2] = at[2];
      for (let spark = 0; spark < HIT_BURST_COUNT; spark++) {
        // Spread round the kart, mostly outward and up.
        const yaw = (spark / HIT_BURST_COUNT) * Math.PI * 2 + random() * 0.3;
        const speed = HIT_BURST_SPEED_M_PER_S * (0.6 + random() * 0.4);
        velocity[0] = Math.cos(yaw) * speed;
        velocity[1] = 2 + random() * 4;
        velocity[2] = Math.sin(yaw) * speed;
        particles.emit(birth);
      }
    },
  };
}
