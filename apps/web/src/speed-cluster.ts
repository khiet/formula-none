// The Instrument panel's speed cluster (ADR 0031): a gear, revs and shift
// lights read off the kart's speed against the top speed its coins raise,
// the coin count (ADR 0056) and the tyre gauge. None of it is a rule; the
// sim has no gearbox and no tyre wear, so the gearing constants are display
// only and live here rather than in the sim's tuning (ADR 0014). The coin
// cap and the top speed on each count of coins are the sim's, passed in.

import { kmPerHour } from "./format";

/** 1 to 6, N while the speed shows as 0 km/h, R while reversing. */
export type Gear = "N" | "R" | "1" | "2" | "3" | "4" | "5" | "6";

/** The gear and revs the cluster shows for a speed. */
export interface Drivetrain {
  readonly gear: Gear;
  readonly rpm: number;
  /** Over top speed, as off a mini-turbo or a boost pad: 6th, held on the limiter. */
  readonly limiter: boolean;
}

/** What one shift light shows: unlit, or its third's colour as on an F1 wheel. */
export type ShiftLight = "off" | "green" | "red" | "blue";

/** The sim's tuning the cluster reads (ADR 0014): its top speeds in m/s and its coins. */
export interface SpeedClusterTuning {
  /** The top speed on each count of coins carried, from none to the cap. */
  readonly coinTopSpeeds: readonly number[];
  /** The most coins a kart carries. */
  readonly coinCap: number;
}

/** How worn the tyres read: green, yellow or red. */
export type TyreWearBand = "good" | "warn" | "bad";

/** The tyre gauge's reading until tyre wear exists in the sim: fresh tyres. */
export const TYRE_WEAR_PLACEHOLDER_PERCENT = 100;

const GEARS = 6;
const IDLE_RPM = 4000;
const SHIFT_RPM = 11_500;
const LIMITER_RPM = 12_000;
const SHIFT_LIGHT_COUNT = 12;
/** The revs the first shift light comes on at; the last lights at the shift. */
const SHIFT_LIGHTS_FROM_RPM = 6000;

/**
 * The gear and revs for `speed` against `topSpeed`, both in m/s: six even
 * bands up to top speed, revs climbing across each. A reverse revs across
 * the first band. The limiter comes on once the shown km/h passes top
 * speed's, so 30.1 m/s against 30 still reads 108 km/h in 6th.
 */
export function drivetrain(speed: number, topSpeed: number): Drivetrain {
  const band = topSpeed / GEARS;
  const shown = Math.round(kmPerHour(speed));
  if (shown === 0) return { gear: "N", rpm: IDLE_RPM, limiter: false };
  const revs = (fraction: number) => IDLE_RPM + Math.min(fraction, 1) * (SHIFT_RPM - IDLE_RPM);
  if (speed < 0) return { gear: "R", rpm: revs(-speed / band), limiter: false };
  if (shown > Math.round(kmPerHour(topSpeed))) {
    return { gear: "6", rpm: LIMITER_RPM, limiter: true };
  }
  const gear = Math.min(GEARS, 1 + Math.floor(speed / band));
  return {
    gear: String(gear) as Gear,
    rpm: revs((speed - (gear - 1) * band) / band),
    limiter: false,
  };
}

/** The row of shift lights for a reading, first to last: lit by revs, all blue on the limiter. */
export function shiftLights(reading: Drivetrain): ShiftLight[] {
  const lit = Math.round(
    ((reading.rpm - SHIFT_LIGHTS_FROM_RPM) / (SHIFT_RPM - SHIFT_LIGHTS_FROM_RPM)) *
      SHIFT_LIGHT_COUNT,
  );
  return Array.from({ length: SHIFT_LIGHT_COUNT }, (_, index): ShiftLight => {
    if (reading.limiter) return "blue";
    if (index >= lit) return "off";
    const third = Math.floor((index * 3) / SHIFT_LIGHT_COUNT);
    return third === 0 ? "green" : third === 1 ? "red" : "blue";
  });
}

/** The tyre gauge's colour for a wear percentage: green above 60%, yellow above 30%, red below. */
export function tyreWearBand(percent: number): TyreWearBand {
  return percent > 60 ? "good" : percent > 30 ? "warn" : "bad";
}

/**
 * The top speed a kart carrying `coins` reaches, as the sim tunes it; the
 * gears stretch to it so the limiter still means a boost.
 */
export function coinTopSpeed(tuning: SpeedClusterTuning, coins: number): number {
  const speeds = tuning.coinTopSpeeds;
  return speeds[Math.min(Math.max(coins, 0), speeds.length - 1)] ?? 0;
}

/** The coin count as the panel prints it: the coins carried in two digits over the cap. */
export function coinCount(coins: number, cap: number): string {
  return `x${String(coins).padStart(2, "0")} / ${cap}`;
}
