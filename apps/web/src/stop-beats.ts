// The beats of a stop in a pit box (ADR 0082), on the stop clock: seconds
// since the kart stood on its mark, negative through the guide onto it and
// past the stationary time after green. The crew's stance and the kart on
// its jacks read them here, and so will the stop's sound cues, so a pose and
// its sound never part. Presentation only: the sim rolls the stop and
// nothing here enters it.

import type { Stop, StopTier } from "./bridge";

/** The light holds green this long after the kart is let go. */
export const GREEN_HOLD_S = 3;

/** The stationary time the beats are set for: a clean stop's middle. */
export const BEATS_SET_FOR_S = 2.65;

export type BeatName =
  | "jackUp"
  | "gunsOff"
  | "wheelsOff"
  | "wheelsOn"
  | "gunsOn"
  | "done"
  | "jackDown";

export interface Beat {
  readonly name: BeatName;
  /** On the stop clock at a stop of `BEATS_SET_FOR_S`; from green when `fromGreen`. */
  readonly startS: number;
  readonly endS: number;
  readonly fromGreen: boolean;
}

/**
 * The stop's beats, in order. Those on the mark are scaled by the
 * stationary time below `BEATS_SET_FOR_S`, so a record stop plays them all
 * faster and a longer one holds the hands up; the jacks drop on the clock
 * to green.
 */
export const STOP_BEATS: readonly Beat[] = [
  { name: "jackUp", startS: 0, endS: 0.4, fromGreen: false },
  { name: "gunsOff", startS: 0.25, endS: 0.85, fromGreen: false },
  { name: "wheelsOff", startS: 0.85, endS: 1.25, fromGreen: false },
  { name: "wheelsOn", startS: 1.25, endS: 1.65, fromGreen: false },
  { name: "gunsOn", startS: 1.65, endS: 2.15, fromGreen: false },
  { name: "done", startS: 2.15, endS: 2.15, fromGreen: false },
  { name: "jackDown", startS: -0.3, endS: -0.05, fromGreen: true },
];

/** Each beat's start and end on the stop clock for a stop of `stationaryS`. */
export type Beats = Readonly<Record<BeatName, readonly [number, number]>>;

export function beatsOf(stationaryS: number): Beats {
  const scale = Math.min(1, stationaryS / BEATS_SET_FOR_S);
  return Object.fromEntries(
    STOP_BEATS.map(({ name, startS, endS, fromGreen }) => [
      name,
      fromGreen ? [stationaryS + startS, stationaryS + endS] : [startS * scale, endS * scale],
    ]),
  ) as unknown as Beats;
}

/**
 * The tier's act past a clean stop's longest, on the stop clock, or null
 * for a stop that has none. A slow stop keeps one gunner on a nut that will
 * not come; a botched one swarms that wheel; a disaster drops the front jack
 * and redoes the front wheels once it is pumped back up.
 */
export type Act =
  | { readonly kind: "slow"; readonly stall: readonly [number, number] }
  | {
      readonly kind: "botched";
      readonly stall: readonly [number, number];
      readonly swarmAt: number;
    }
  | {
      readonly kind: "disaster";
      readonly failAt: number;
      readonly rejack: readonly [number, number];
      readonly redo: readonly [number, number];
    };

/** Seconds after the redo starts: nuts off until the first, pulled until the second, pushed until the third, nuts on after. */
export const REDO_STEPS_S = [0.5, 0.7, 0.9] as const;

export function actOf(tier: StopTier, stationaryS: number, actFromS: number): Act | null {
  // Freed half a second before green, or half way through a short act, so every slow stop shows its stall.
  const freed =
    stationaryS - 0.5 > actFromS ? stationaryS - 0.5 : actFromS + (stationaryS - actFromS) / 2;
  switch (tier) {
    case "slow":
      return { kind: "slow", stall: [actFromS, freed] };
    case "botched":
      return { kind: "botched", stall: [actFromS, freed], swarmAt: actFromS + 0.25 };
    case "disaster":
      return {
        kind: "disaster",
        failAt: actFromS,
        rejack: [stationaryS - 3, stationaryS - 2.6],
        redo: [stationaryS - 2.5, stationaryS - 1.1],
      };
    default:
      return null;
  }
}

/** The release, on the stop clock from green. */
export const RELEASE = {
  /** The holder looks down the lane for traffic. */
  laneCheck: [-1, -0.55],
  /** The sign spins from STOP to GO. */
  spin: [-0.6, -0.2],
  /** The holder steps aside out of the kart's path, landing before green. */
  stepAside: -0.25,
  /** The sign lifts. */
  lift: [0, 0.3],
  /** The driver's eyes leave the sign for the road. */
  eyesAhead: -0.5,
} as const;

/** The crew walks back to its row this long after the hold, and is back by this. */
export const CREW_BACK_S = 2.5;

/** The jacks lift the kart this far. */
export const KART_LIFT_M = 0.16;

export const clamp01 = (x: number) => Math.min(1, Math.max(0, x));
/** 0 before `from`, 1 after `to`, and linear between. */
export const ramp = (t: number, from: number, to: number) => clamp01((t - from) / (to - from));

/**
 * How far each end of the kart is up on its jack, 0 on the ground and 1 at
 * `KART_LIFT_M`, for `stop`, its kart's stop or null; `actFromS` is a clean
 * stop's longest. The front comes up a beat ahead of the rear; on a
 * disaster the front drops, bounces and comes up again on the rejack.
 */
export function kartOnJacks(
  stop: Stop | null,
  actFromS: number,
): { readonly front: number; readonly rear: number } {
  if (stop === null || stop.guideLeftS > 0) return { front: 0, rear: 0 };
  const t = stop.stationarySoFarS;
  const beats = beatsOf(stop.stationaryS);
  const [upFrom, upTo] = beats.jackUp;
  const down = 1 - ramp(t, ...beats.jackDown);
  let front = ramp(t, upFrom, upTo);
  const rear = ramp(t, upFrom + 0.05, upTo + 0.05) * down;
  const act = actOf(stop.tier, stop.stationaryS, actFromS);
  if (act?.kind === "disaster" && t > act.failAt) {
    const dropped = 1 - ramp(t, act.failAt, act.failAt + 0.25);
    const bounce =
      t < act.failAt + 0.6
        ? Math.abs(Math.sin((t - act.failAt) * 20)) *
          0.25 *
          (1 - ramp(t, act.failAt, act.failAt + 0.6))
        : 0;
    front = Math.max(dropped, ramp(t, ...act.rejack)) + bounce;
  }
  return { front: Math.min(1, front * down), rear };
}

/**
 * How a kart of `kartLength` on `stop` sits on its jacks: its middle
 * `liftM` up and its nose `pitch` radians up, about its middle.
 */
export function kartLift(
  stop: Stop | null,
  actFromS: number,
  kartLength: number,
): { readonly liftM: number; readonly pitch: number } {
  const { front, rear } = kartOnJacks(stop, actFromS);
  // The axles are a little over half the kart's length apart.
  const wheelbase = kartLength * 0.58;
  return {
    liftM: ((front + rear) / 2) * KART_LIFT_M,
    pitch: Math.atan2((front - rear) * KART_LIFT_M, wheelbase),
  };
}

/** How far the driver turns their head to the sign, radians toward the pit box's building. */
export const DRIVER_LOOK_RAD = 0.28;

/**
 * How far toward the sign the driver of a kart on `stop` looks, 0 ahead and
 * 1 at `DRIVER_LOOK_RAD`: on it from the mark until the sign spins, with a
 * quick turn either side.
 */
export function driverLook(stop: Stop | null): number {
  if (stop === null || stop.guideLeftS > 0) return 0;
  const t = stop.stationarySoFarS;
  const ahead = stop.stationaryS + RELEASE.eyesAhead;
  return ramp(t, 0, 0.2) * (1 - ramp(t, ahead, ahead + 0.2));
}
