import type { KartReadout, KartSize, MiniTurbo } from "./bridge";
import type { Start } from "./generated/protocol";
import { rearWheel, rotate, type Side, type Vec3 } from "./kart-frame";
import { createParticles, type Particles } from "./particles";

/** A charged tier's sparks, the mini-turbo charge's tell at a kart's wheels. */
export interface SparkTell {
  readonly tier: Exclude<MiniTurbo, "none">;
  /** A Three.js colour hex: the spark at birth, white-hot. */
  readonly colour: number;
  /** A Three.js colour hex: the tier's own colour, which a spark cools to. */
  readonly cooledColour: number;
  /** Sparks thrown per wheel per second while the tier is charged. */
  readonly perWheelPerSecond: number;
  /** How fast a spark leaves the wheel, straight back along the kart. */
  readonly speedMetresPerSecond: number;
  readonly lifetimeSeconds: number;
  /** Whether each spark has a halo. */
  readonly glow: boolean;
  /** The share of its speed a spark keeps bouncing off the road; null when it does not bounce. */
  readonly restitution: number | null;
  /** Sparks thrown, across both wheels, on the frame the tier charges. */
  readonly burst: number;
  /** The most a burst spark's path strays sideways, wider than the steady sparks'. */
  readonly burstSpreadMetresPerSecond: number;
  readonly burstRiseMaxMetresPerSecond: number;
}

/**
 * Each charged tier's colours: what a spark is born and what it cools to.
 * A boost's exhaust flame burns in them too, so a red release still says
 * red after the sparks stop.
 */
export const TIER_COLOURS: Readonly<
  Record<Exclude<MiniTurbo, "none">, { readonly hot: number; readonly cooled: number }>
> = {
  orange: { hot: 0xfff0b0, cooled: 0xff7a00 },
  red: { hot: 0xffd27a, cooled: 0xe01808 },
};

/**
 * Each tier's sparks: orange quiet, red the Grinder, faster, longer-lived,
 * glowing, bouncing and bursting harder, so the second tier reads stronger
 * and not only different. Both are born hot and cool to the tier's colour.
 */
const SPARK_TELLS: Readonly<Record<MiniTurbo, SparkTell | null>> = {
  none: null,
  orange: {
    tier: "orange",
    colour: TIER_COLOURS.orange.hot,
    cooledColour: TIER_COLOURS.orange.cooled,
    perWheelPerSecond: 60,
    speedMetresPerSecond: 6,
    lifetimeSeconds: 0.25,
    glow: false,
    restitution: null,
    burst: 6,
    burstSpreadMetresPerSecond: 3,
    burstRiseMaxMetresPerSecond: 3.5,
  },
  red: {
    tier: "red",
    colour: TIER_COLOURS.red.hot,
    cooledColour: TIER_COLOURS.red.cooled,
    perWheelPerSecond: 90,
    speedMetresPerSecond: 7.5,
    lifetimeSeconds: 0.35,
    glow: true,
    restitution: 0.4,
    burst: 20,
    burstSpreadMetresPerSecond: 4,
    burstRiseMaxMetresPerSecond: 4.5,
  },
};

/**
 * The mini-turbo tier a rocket start's charge sparks like: the weak boost
 * as orange, the full boost as red, and the stall as red too, so the
 * hold tipping into a stall gives no warning.
 */
const CHARGE_TIERS: Readonly<Record<Start, MiniTurbo>> = {
  clean: "none",
  weakBoost: "orange",
  fullBoost: "red",
  stall: "red",
};

/**
 * The Feedback for a mini-turbo charge and a rocket start's (GLOSSARY):
 * sparks at the wheels in orange's style once orange has charged and red's
 * once red has, and nothing before a tier charges or off a drift; and,
 * while the countdown runs, in the style of the tier the hold so far has
 * reached, which `charge` is. Presentation only; reads state.
 */
export function sparkTell(
  kart: Pick<KartReadout, "drift" | "miniTurbo">,
  charge: Start | null = null,
): SparkTell | null {
  const tier =
    kart.drift !== null ? kart.miniTurbo : charge === null ? "none" : CHARGE_TIERS[charge];
  return SPARK_TELLS[tier];
}

/**
 * Sparks live at once across the grid: every kart on the red tier, both
 * wheels through a red spark's life, plus each kart's red burst, with room
 * over. A pool full earlier drops its oldest.
 */
const SPARK_CAPACITY_PER_KART = 96;
/** Sparks fall like embers. */
const SPARK_GRAVITY_M_PER_S2 = 9.8;
/** A spark has cooled to its tier's colour by this share of its life. */
const SPARK_COOL_SHARE = 0.8;
/** A spark is drawn as a streak this many seconds of its travel long. */
const SPARK_STREAK_S = 0.03;
/** The share of its speed along the road a bouncing spark keeps. */
const SPARK_BOUNCE_SLIDE = 0.7;

/** The pool every kart's sparks are thrown into, sized for `kartCount` karts. */
export function createSparkParticles(kartCount: number): Particles {
  return createParticles({
    capacity: kartCount * SPARK_CAPACITY_PER_KART,
    gravity: SPARK_GRAVITY_M_PER_S2,
    coolShare: SPARK_COOL_SHARE,
    stretchSeconds: SPARK_STREAK_S,
    bounceSlide: SPARK_BOUNCE_SLIDE,
  });
}

export interface SparkEmitter {
  /**
   * Throw this frame's sparks for `kart` into the pool, if its drift has a
   * tier charged or, while `countingDown`, its rocket start has, and a
   * burst on the frame the tier rises.
   */
  emit(kart: KartReadout, dtSeconds: number, countingDown?: boolean): void;
}

/** The most a steady spark's path strays sideways and the range it kicks up, in metres per second. */
const SPARK_SPREAD_M_PER_S = 2;
const SPARK_RISE_MIN_M_PER_S = 1;
const SPARK_RISE_MAX_M_PER_S = 3;
/** A burst's sparks leave the wheel between these speeds. */
const BURST_SPEED_MIN_M_PER_S = 9;
const BURST_SPEED_MAX_M_PER_S = 11;
const SPARK_SIZE_M = 0.08;
const SIDES: readonly Side[] = [-1, 1];
/** The most emission a frame can owe, so a stalled tab does not empty the pool in one burst. */
const MAX_OWED_S = 0.1;
const TIER_RANK: Readonly<Record<MiniTurbo, number>> = { none: 0, orange: 1, red: 2 };

/**
 * Throws sparks off the rear wheels of one kart whose drift or rocket
 * start has charged a tier, at the tier's steady rate whatever the frame
 * rate, backward and up from the tyre's print and in the tier's style,
 * and a burst, faster and wider, on the frame the tier rises. Remembers
 * the kart's last tier, so each kart has an emitter of its own. `random`
 * is Math.random's shape; tests pass a fixed sequence. Nothing is
 * allocated per frame.
 */
export function createSparkEmitter(
  particles: Particles,
  size: KartSize,
  random: () => number = Math.random,
): SparkEmitter {
  /** Sparks owed and not yet thrown; a fraction carries to the next frame. */
  let owed = 0;
  let lastTier: MiniTurbo = "none";
  const position: Vec3 = [0, 0, 0];
  const velocity: Vec3 = [0, 0, 0];
  const back: Vec3 = [0, 0, 0];
  const across: Vec3 = [0, 0, 0];
  const birth = {
    position,
    velocity,
    colour: 0,
    cooledColour: 0,
    lifetimeSeconds: 0,
    size: SPARK_SIZE_M,
    restitution: null as number | null,
    glow: false,
  };
  const throwSpark = (
    kart: KartReadout,
    side: Side,
    speed: number,
    spread: number,
    riseMax: number,
  ) => {
    rearWheel(kart, side, size, position);
    const stray = (2 * random() - 1) * spread;
    const rise = SPARK_RISE_MIN_M_PER_S + random() * (riseMax - SPARK_RISE_MIN_M_PER_S);
    velocity[0] = back[0] * speed + across[0] * stray;
    velocity[1] = back[1] * speed + across[1] * stray + rise;
    velocity[2] = back[2] * speed + across[2] * stray;
    particles.emit(birth);
  };
  return {
    emit(kart, dtSeconds, countingDown = false) {
      const tell = sparkTell(kart, countingDown ? kart.start : null);
      const rose = tell !== null && TIER_RANK[tell.tier] > TIER_RANK[lastTier];
      lastTier = tell?.tier ?? "none";
      if (tell === null) {
        return;
      }
      // A stalled frame throws a burst, not a frame's worth of the stall.
      owed = Math.min(
        owed + tell.perWheelPerSecond * dtSeconds,
        tell.perWheelPerSecond * MAX_OWED_S,
      );
      if (owed < 1 && !rose) {
        return;
      }
      birth.colour = tell.colour;
      birth.cooledColour = tell.cooledColour;
      birth.lifetimeSeconds = tell.lifetimeSeconds;
      birth.restitution = tell.restitution;
      birth.glow = tell.glow;
      rotate(kart.rotation, 0, 0, 1, back);
      rotate(kart.rotation, 1, 0, 0, across);
      if (rose) {
        for (let spark = 0; spark < tell.burst; spark++) {
          const speed =
            BURST_SPEED_MIN_M_PER_S +
            random() * (BURST_SPEED_MAX_M_PER_S - BURST_SPEED_MIN_M_PER_S);
          const side = SIDES[spark % SIDES.length] ?? 1;
          throwSpark(
            kart,
            side,
            speed,
            tell.burstSpreadMetresPerSecond,
            tell.burstRiseMaxMetresPerSecond,
          );
        }
      }
      for (; owed >= 1; owed -= 1) {
        for (const side of SIDES) {
          throwSpark(
            kart,
            side,
            tell.speedMetresPerSecond,
            SPARK_SPREAD_M_PER_S,
            SPARK_RISE_MAX_M_PER_S,
          );
        }
      }
    },
  };
}
