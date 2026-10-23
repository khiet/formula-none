import type { Drift, KartSize, KartTransform, MiniTurbo } from "./bridge";
import { rearWheel, rotate, type Side, type Vec3 } from "./kart-frame";
import type { Particles } from "./particles";

/** The spark tell at a kart's wheels: shown once a mini-turbo tier has charged, coloured by the tier. */
export interface SparkTell {
  /** A Three.js colour hex. */
  readonly colour: number;
}

/** Spark colour by tier; an uncharged drift has no sparks, only its tyre marks. */
const SPARK_COLOURS: Readonly<Record<MiniTurbo, number | null>> = {
  none: null,
  blue: 0x3399ff,
  orange: 0xff8800,
};

/**
 * The Feedback for a mini-turbo charge (GLOSSARY): sparks at the wheels in
 * blue once blue has charged and orange once orange has, and nothing
 * before a tier charges or off a drift. Presentation only; reads state.
 */
export function sparkTell(drift: Drift | null): SparkTell | null {
  const colour = drift === null ? null : SPARK_COLOURS[drift.miniTurbo];
  return colour === null ? null : { colour };
}

export interface SparkEmitter {
  /** Throw this frame's sparks for `kart` into the pool, if its drift has a tier charged. */
  emit(kart: KartTransform, dtSeconds: number): void;
}

/** Sparks thrown per wheel per second while a tier is charged. */
const SPARKS_PER_WHEEL_PER_S = 60;
/** How fast a spark leaves the wheel, straight back along the kart. */
const SPARK_SPEED_M_PER_S = 6;
/** The most a spark's path strays sideways and the range it kicks up, in metres per second. */
const SPARK_SPREAD_M_PER_S = 2;
const SPARK_RISE_MIN_M_PER_S = 1;
const SPARK_RISE_MAX_M_PER_S = 3;
const SPARK_LIFETIME_S = 0.25;
const SPARK_SIZE_M = 0.08;
const SIDES: readonly Side[] = [-1, 1];

/**
 * Throws sparks off the rear wheels of a kart whose drift has charged a
 * tier, at a steady rate whatever the frame rate, backward and up from the
 * tyre's print and in the tier's colour. `random` is Math.random's shape;
 * tests pass a fixed sequence. Nothing is allocated per frame.
 */
export function createSparkEmitter(
  particles: Particles,
  size: KartSize,
  random: () => number = Math.random,
): SparkEmitter {
  /** Sparks owed and not yet thrown; a fraction carries to the next frame. */
  let owed = 0;
  const position: Vec3 = [0, 0, 0];
  const velocity: Vec3 = [0, 0, 0];
  const back: Vec3 = [0, 0, 0];
  const across: Vec3 = [0, 0, 0];
  const birth = {
    position,
    velocity,
    colour: 0,
    lifetimeSeconds: SPARK_LIFETIME_S,
    size: SPARK_SIZE_M,
  };
  return {
    emit(kart, dtSeconds) {
      const tell = sparkTell(kart.drift);
      if (tell === null) {
        return;
      }
      owed += SPARKS_PER_WHEEL_PER_S * dtSeconds;
      if (owed < 1) {
        return;
      }
      birth.colour = tell.colour;
      rotate(kart.rotation, 0, 0, 1, back);
      rotate(kart.rotation, 1, 0, 0, across);
      for (; owed >= 1; owed -= 1) {
        for (const side of SIDES) {
          rearWheel(kart, side, size, position);
          const stray = (2 * random() - 1) * SPARK_SPREAD_M_PER_S;
          const rise =
            SPARK_RISE_MIN_M_PER_S + random() * (SPARK_RISE_MAX_M_PER_S - SPARK_RISE_MIN_M_PER_S);
          velocity[0] = back[0] * SPARK_SPEED_M_PER_S + across[0] * stray;
          velocity[1] = back[1] * SPARK_SPEED_M_PER_S + across[1] * stray + rise;
          velocity[2] = back[2] * SPARK_SPEED_M_PER_S + across[2] * stray;
          particles.emit(birth);
        }
      }
    },
  };
}
