// The runaway wheel's tells, as docs/mockups/runaway-wheel-danger-spec.html
// draws them: its flames point off its back, it
// trails smoke, it kicks up dust where it lands from each drawn bounce and
// where it rebounds off a wall, and strikes sparks off that wall.
// Presentation only, decided from the wheels the bridge reports: a wall hit
// is where the leg under way starts over, and a drawn bounce lands where
// `runawayWheelLanded` says.

import type { Wheel } from "./bridge";
import { SAND_DUST } from "./dust";
import { runawayWheelLanded } from "./item-meshes";
import type { Vec3 } from "./kart-frame";
import { ROCKET_FLAME, WHEEL_SMOKE } from "./palette";
import type { Particles } from "./particles";

/**
 * Straight back from the way a wheel at `yaw` rolls, level and unit length,
 * into `out`: where its flames point and its smoke is left. Zero yaw rolls
 * toward -Z.
 */
export function wheelBack(yaw: number, out: Vec3): Vec3 {
  out[0] = Math.sin(yaw);
  out[1] = 0;
  out[2] = Math.cos(yaw);
  return out;
}

/** The pools the wheel's tells are thrown into. */
export interface WheelTellPools {
  /** The run-off dust's pool. */
  readonly dust: Particles;
  /** The smoke's pool; the View passes the dust's, so the puffs spread and fade alike. */
  readonly smoke: Particles;
  /** The sparks' pool, under gravity, its sparks streaked. */
  readonly sparks: Particles;
}

export interface WheelTellEmitter {
  /** Throw this frame's smoke, dust and sparks for each crossing's wheel in `wheels`. */
  emit(wheels: readonly Wheel[], dtSeconds: number): void;
}

/** Smoke puffs a second off a rolling wheel, rising and growing behind it. */
const SMOKE_PUFFS_PER_S = 24;
const SMOKE_RISE_M_PER_S = 1;
const SMOKE_LIFETIME_S = 0.9;
const SMOKE_SIZE_M = 0.6;
/** The most smoke a frame can owe, so a stalled tab does not empty the pool in one burst. */
const MAX_OWED_S = 0.1;
/** A landing's or a wall hit's dust: puffs round where the tyre meets the road. */
const DUST_PUFFS = 5;
const DUST_SPREAD_M_PER_S = 2;
const DUST_RISE_M_PER_S = 0.6;
const DUST_LIFETIME_S = 0.45;
const DUST_SIZE_M = 0.5;
/** A wall hit's sparks fly 1.5 m off the wall over their life. */
const SPARKS = 8;
const SPARK_LIFETIME_S = 0.3;
const SPARK_SPEED_M_PER_S = 1.5 / SPARK_LIFETIME_S;
/** The most a spark strays along the wall, and the range it kicks up, in metres per second. */
const SPARK_SPREAD_M_PER_S = 3;
const SPARK_RISE_MIN_M_PER_S = 1;
const SPARK_RISE_MAX_M_PER_S = 3;
/** Twice a kart's spark, and haloed, so they read beside a wheel 2.4 m across. */
const SPARK_SIZE_M = 0.16;

/** What a crossing's wheel was last frame; unset while it is off the course. */
interface Last {
  legFraction: number;
  yaw: number;
}

/**
 * Throws the tells of every crossing's wheel `radius` in size: smoke off
 * its back at a steady rate whatever the frame rate while it rolls, a
 * burst of dust where it lands from each bounce and each wall hit, and a
 * burst of sparks off the wall where its tyre strikes at each wall hit,
 * flying back over the track. Remembers each wheel's last frame to find
 * those moments. `random` is Math.random's shape; tests pass a fixed value.
 */
export function createWheelTellEmitter(
  pools: WheelTellPools,
  radius: number,
  random: () => number = Math.random,
): WheelTellEmitter {
  const last: (Last | undefined)[] = [];
  const owed: number[] = [];
  const position: Vec3 = [0, 0, 0];
  const velocity: Vec3 = [0, 0, 0];
  const back: Vec3 = [0, 0, 0];
  const before: Vec3 = [0, 0, 0];
  const away: Vec3 = [0, 0, 0];
  const smoke = {
    position,
    velocity,
    colour: WHEEL_SMOKE,
    lifetimeSeconds: SMOKE_LIFETIME_S,
    size: SMOKE_SIZE_M,
  };
  const dust = {
    position,
    velocity,
    colour: SAND_DUST,
    lifetimeSeconds: DUST_LIFETIME_S,
    size: DUST_SIZE_M,
  };
  const spark = {
    position,
    velocity,
    colour: ROCKET_FLAME,
    lifetimeSeconds: SPARK_LIFETIME_S,
    size: SPARK_SIZE_M,
    glow: true,
  };

  const kickDust = (at: Readonly<Vec3>) => {
    for (let puff = 0; puff < DUST_PUFFS; puff++) {
      // Spread evenly round the print, nudged by `random`.
      const turn = (2 * Math.PI * (puff + random())) / DUST_PUFFS;
      position[0] = at[0];
      position[1] = at[1];
      position[2] = at[2];
      velocity[0] = Math.cos(turn) * DUST_SPREAD_M_PER_S;
      velocity[1] = DUST_RISE_M_PER_S;
      velocity[2] = Math.sin(turn) * DUST_SPREAD_M_PER_S;
      pools.dust.emit(dust);
    }
  };

  // The turn a rebound makes, the new back from the old, stands in for the
  // wall's normal toward the track; the seeded landings skew it along the track.
  const strikeSparks = (wheel: Extract<Wheel, { kind: "rolling" }>, yawBefore: number) => {
    wheelBack(yawBefore, before);
    wheelBack(wheel.yaw, back);
    const length = Math.hypot(before[0] - back[0], before[2] - back[2]);
    if (length < 1e-6) return;
    away[0] = (before[0] - back[0]) / length;
    away[2] = (before[2] - back[2]) / length;
    const [x, y, z] = wheel.position;
    for (let thrown = 0; thrown < SPARKS; thrown++) {
      // Struck at axle height, where the tyre's tread meets the wall.
      position[0] = x - away[0] * radius;
      position[1] = y + radius;
      position[2] = z - away[2] * radius;
      const stray = (2 * random() - 1) * SPARK_SPREAD_M_PER_S;
      velocity[0] = away[0] * SPARK_SPEED_M_PER_S - away[2] * stray;
      velocity[1] =
        SPARK_RISE_MIN_M_PER_S + random() * (SPARK_RISE_MAX_M_PER_S - SPARK_RISE_MIN_M_PER_S);
      velocity[2] = away[2] * SPARK_SPEED_M_PER_S + away[0] * stray;
      pools.sparks.emit(spark);
    }
  };

  return {
    emit(wheels, dtSeconds) {
      wheels.forEach((wheel, crossing) => {
        if (wheel.kind !== "rolling") {
          last[crossing] = undefined;
          owed[crossing] = 0;
          return;
        }
        const was = last[crossing];
        last[crossing] = { legFraction: wheel.legFraction, yaw: wheel.yaw };
        if (was !== undefined) {
          if (wheel.legFraction < was.legFraction) {
            strikeSparks(wheel, was.yaw);
            kickDust(wheel.position);
          } else if (runawayWheelLanded(was.legFraction, wheel.legFraction)) {
            kickDust(wheel.position);
          }
        }
        // A stalled frame throws a burst, not a frame's worth of the stall.
        let due = Math.min(
          (owed[crossing] ?? 0) + SMOKE_PUFFS_PER_S * dtSeconds,
          SMOKE_PUFFS_PER_S * MAX_OWED_S,
        );
        wheelBack(wheel.yaw, back);
        const [x, y, z] = wheel.position;
        for (; due >= 1; due -= 1) {
          // Off the back of the tyre, at axle height, left to rise where it was.
          position[0] = x + back[0] * radius;
          position[1] = y + radius;
          position[2] = z + back[2] * radius;
          velocity[0] = 0;
          velocity[1] = SMOKE_RISE_M_PER_S;
          velocity[2] = 0;
          pools.smoke.emit(smoke);
        }
        owed[crossing] = due;
      });
    },
  };
}
