// The air rushing past the own kart while it boosts or holds a slipstream
// (ADR 0077): white streaks that come from ahead and part around its sides
// and roof. A boost throws them densest as it begins and fades them over
// its last moments; a wake throws them building from nothing through the
// charge, and a tow at the boost's steady rate, fading as the tow runs out.
// The local player's only, so the effect is felt from the seat; other karts
// show their boosts by the flame and their tows by nothing. Presentation
// only, decided from the own kart's readout.

import type { KartReadout, KartSize, Tuning } from "./bridge";
import type { Slipstream } from "./generated/sim/Slipstream";
import { rotate, toWorld, type Vec3 } from "./kart-frame";
import { createParticles, type Particles } from "./particles";

/** Lines live at once: the densest rate through a line's life, with room over. */
const WIND_CAPACITY = 64;
/** A line is a streak this many seconds of its travel long. */
const WIND_STREAK_S = 0.2;
/** A line is born dim far ahead and brightens as it reaches the kart. */
const WIND_BORN_COLOUR = 0x303030;
const WIND_NEAR_COLOUR = 0xb0b0b0;
/** Long enough to carry every line from ahead of the kart to behind it. */
const WIND_LIFETIME_S = 0.45;
const WIND_SIZE_M = 0.03;
/** Lines thrown per second as a boost has this long left or longer; fewer as it runs down. */
const WIND_DENSE_BOOST_S = 1;
const WIND_RATE_PER_S = 90;
const WIND_LEAST_RATE_PER_S = 40;
/** The share of the lines thrown under reduced motion. */
const WIND_REDUCED_SHARE = 0.25;
/** The lines fade out over the boost's last this many seconds. */
const WIND_FADE_S = 0.35;
/** A line starts this far ahead of the kart's centre. */
const WIND_AHEAD_MIN_M = 9;
const WIND_AHEAD_MAX_M = 14;
/** The share of lines that pass over the roof; the rest pass the sides. */
const WIND_ROOF_SHARE = 0.3;
/**
 * How fast a line closes on the kart, whatever the kart's own speed, so it
 * passes the kart and the camera within its life even at a rocket start;
 * and the least it moves back through the world, so it always streaks.
 */
const WIND_CLOSING_MIN_M_PER_S = 40;
const WIND_CLOSING_MAX_M_PER_S = 50;
const WIND_LEAST_SPEED_M_PER_S = 5;
/** How fast a side line parts outward and a roof line lifts, the way air parts round the kart. */
const WIND_PART_MIN_M_PER_S = 0.8;
const WIND_PART_MAX_M_PER_S = 2;
const WIND_LIFT_M_PER_S = 0.6;
/** How far out from the kart's side a side line passes, past the side itself. */
const WIND_SIDE_GAP_M = 0.2;
const WIND_SIDE_SPREAD_M = 1.6;
/** How high over the ground a side line passes and how far over the roof a roof line does. */
const WIND_SIDE_HEIGHT_M = 1.6;
const WIND_ROOF_HEIGHT_M = 0.9;

/** The pool the own kart's wind lines are thrown into. */
export function createWindParticles(): Particles {
  return createParticles({ capacity: WIND_CAPACITY, stretchSeconds: WIND_STREAK_S });
}

/**
 * How brightly the lines show for a kart with `boostS` seconds of boost
 * left, or else with the tow `slipstream` carries: fully until the last
 * moments, then fading to nothing as it ends. Full through a charge, whose
 * lines are few rather than dim. Nothing off all three.
 */
export function windFade(boostS: number | null, slipstream: Slipstream | null = null): number {
  if (boostS !== null) return fadeOver(boostS);
  if (slipstream === null) return 0;
  return "tow" in slipstream ? fadeOver(slipstream.tow.leftS) : 1;
}

function fadeOver(leftS: number): number {
  return Math.min(Math.max(leftS, 0) / WIND_FADE_S, 1);
}

/**
 * Lines thrown per second for `kart`: a boost's rate, densest as it
 * begins; else a tow's steady rate, the boost's; else a charge's, climbing
 * from nothing to the least boost rate as the charge fills; else none.
 */
export function windRate(
  kart: Pick<KartReadout, "boostS" | "slipstream">,
  chargeS: number,
): number {
  if (kart.boostS !== null) {
    const dense = Math.min(Math.max(kart.boostS, 0) / WIND_DENSE_BOOST_S, 1);
    return WIND_LEAST_RATE_PER_S + (WIND_RATE_PER_S - WIND_LEAST_RATE_PER_S) * dense;
  }
  if (kart.slipstream === null) return 0;
  if ("tow" in kart.slipstream) return WIND_RATE_PER_S;
  return WIND_LEAST_RATE_PER_S * Math.min(Math.max(kart.slipstream.charging.s, 0) / chargeS, 1);
}

/** The sim's seconds a wake takes to charge, which the charge's rate climbs over (ADR 0014). */
export interface WindLinesOptions extends Pick<Tuning, "slipstreamChargeS"> {
  /** The viewer asked for less motion: far fewer lines. */
  readonly reducedMotion?: boolean;
}

export interface WindLines {
  /** Throw this frame's lines round the own `kart` into the pool, while it boosts, charges or is towed. */
  emit(kart: KartReadout, dtSeconds: number): void;
}

/** The most emission a frame can owe, so a stalled tab does not throw every line at once. */
const MAX_OWED_S = 0.1;
/** The owed lines always have room to reach one, so a rate under ten a second, a thin charge under reduced motion, still throws its lines. */
const MIN_OWED = 1;

/**
 * Throws wind lines ahead of the own kart while it boosts, charges or is
 * towed, at `windRate`, each carried back past the kart and parting
 * outward around its sides or lifting over its roof. `random` is
 * Math.random's shape; tests pass a fixed sequence.
 */
export function createWindLines(
  particles: Particles,
  size: KartSize,
  options: WindLinesOptions,
  random: () => number = Math.random,
): WindLines {
  const share = options.reducedMotion === true ? WIND_REDUCED_SHARE : 1;
  const chargeS = options.slipstreamChargeS;
  let owed = 0;
  const position: Vec3 = [0, 0, 0];
  const velocity: Vec3 = [0, 0, 0];
  const birth = {
    position,
    velocity,
    colour: WIND_BORN_COLOUR,
    cooledColour: WIND_NEAR_COLOUR,
    lifetimeSeconds: WIND_LIFETIME_S,
    size: WIND_SIZE_M,
  };
  const between = (least: number, most: number) => least + random() * (most - least);
  return {
    emit(kart, dtSeconds) {
      const rate = share * windRate(kart, chargeS);
      if (rate === 0) {
        owed = 0;
        return;
      }
      owed = Math.min(owed + rate * dtSeconds, Math.max(rate * MAX_OWED_S, MIN_OWED));
      const ground = -size.height / 2;
      for (; owed >= 1; owed -= 1) {
        const ahead = -between(WIND_AHEAD_MIN_M, WIND_AHEAD_MAX_M);
        const back = Math.max(
          between(WIND_CLOSING_MIN_M_PER_S, WIND_CLOSING_MAX_M_PER_S) - Math.max(kart.speed, 0),
          WIND_LEAST_SPEED_M_PER_S,
        );
        if (random() < WIND_ROOF_SHARE) {
          const x = (2 * random() - 1) * (size.width / 2 + WIND_SIDE_GAP_M);
          const y = size.height / 2 + random() * WIND_ROOF_HEIGHT_M;
          toWorld(kart, x, y, ahead, position);
          rotate(kart.rotation, 0, WIND_LIFT_M_PER_S, back, velocity);
        } else {
          const side = random() < 0.5 ? -1 : 1;
          const x = side * (size.width / 2 + WIND_SIDE_GAP_M + random() * WIND_SIDE_SPREAD_M);
          const y = ground + random() * WIND_SIDE_HEIGHT_M;
          toWorld(kart, x, y, ahead, position);
          const part = side * between(WIND_PART_MIN_M_PER_S, WIND_PART_MAX_M_PER_S);
          rotate(kart.rotation, part, 0, back, velocity);
        }
        particles.emit(birth);
      }
    },
  };
}
