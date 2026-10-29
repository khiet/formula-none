import type { CameraTuning, KartReadout } from "./bridge";
import { shortestTurn, yawOf } from "./kart-frame";

/** Where the Race View's camera sits and what it looks at, in world metres. */
export interface CameraPose {
  readonly position: readonly [number, number, number];
  readonly target: readonly [number, number, number];
  /** The vertical field of view in degrees. */
  readonly fieldOfView: number;
}

export interface ChaseCameraOptions {
  /** The viewer asked for less motion: no kick on a boost. */
  readonly reducedMotion?: boolean;
}

export interface ChaseCamera {
  /**
   * The pose for this frame, `dtSeconds` after the last one, following
   * `kart`; facing back at it from ahead while `lookingBehind`.
   */
  update(kart: KartReadout, dtSeconds: number, lookingBehind?: boolean): CameraPose;
}

const BEHIND_M = 6;
// High enough that the line of sight to the look-ahead point clears the
// driver's head, a metre over the kart's centre, so the own kart never
// hides the road ahead.
const ABOVE_M = 3.2;
const LOOK_AHEAD_M = 4;
/**
 * A spring on the camera's yaw toward the kart's. Critically damped, so it
 * settles behind the kart without swinging past it.
 */
const YAW_SPRING_STIFFNESS = 20;
const YAW_SPRING_DAMPING = 2 * Math.sqrt(YAW_SPRING_STIFFNESS);
/**
 * A spring on the camera's height toward the kart's, critically damped like
 * the yaw's. Soft enough that a 0.3 s hop lifts the camera by under a third
 * of the kart's rise, so the kart climbs in the frame; stiff enough that the
 * camera trails a 2 m climb over 3 s by under a metre and settles at the new
 * height within about two seconds. Up Suzuqa Patch's 5 m bridge at a boosted
 * 34 m/s it lags to 1.2 m over the kart's centre, still clear of the
 * driver's head; under the deck it rides 0.4 m below the underside.
 */
const HEIGHT_SPRING_STIFFNESS = 16;
const HEIGHT_SPRING_DAMPING = 2 * Math.sqrt(HEIGHT_SPRING_STIFFNESS);
/** The spring integrates in steps no longer than this, so a stalled frame cannot fling it. */
const MAX_SPRING_STEP_S = 1 / 60;
/**
 * The shake's three axes oscillate at these rates in hertz: unequal and
 * well under half the frame rate, so the jolt reads as a rattle rather
 * than a pattern or a strobe.
 */
const SHAKE_HZ: readonly [number, number, number] = [13, 17, 11];

/**
 * A third-person camera behind and above the local kart. Its yaw lags the
 * kart's through a damped spring, so a corner swings the kart across the
 * frame before the camera follows, and it looks a little ahead of the kart
 * along the kart's own heading. Its height lags the kart's through another,
 * so a hop lifts the kart in the frame while a slope is followed. It sits
 * further back the faster the kart goes, the tuned pull-back at top speed,
 * and rattles for the tuned time when the sim reports the kart hit, that is
 * when its spin-out begins. A boost widens its field of view, the kick,
 * unless `options` asks for reduced motion. Looking behind cuts to the
 * mirror of that pose, ahead of the kart and facing back; the springs run on through it, so
 * releasing cuts straight back to the chase pose with no swing.
 * Presentation only: it reads transforms the sim reported and never touches
 * the sim.
 */
export function createChaseCamera(
  tuning: CameraTuning,
  options: ChaseCameraOptions = {},
): ChaseCamera {
  const reducedMotion = options.reducedMotion ?? false;
  let yaw: number | undefined;
  let yawVelocity = 0;
  /** The height the camera follows: the kart's, lagged, in world metres. */
  let height: number | undefined;
  let heightVelocity = 0;
  /** Whether the kart was spun out last frame; a spin-out beginning is the hit. */
  let wasSpunOut: boolean | undefined;
  /** Seconds into the shake, or past its end when there is none. */
  let shakeAge = tuning.shakeSeconds;
  return {
    update(kart, dtSeconds, lookingBehind = false) {
      const kartYaw = yawOf(kart.rotation);
      const [x, kartY, z] = kart.position;
      if (yaw === undefined || height === undefined) {
        yaw = kartYaw;
        height = kartY;
      }
      for (let remaining = dtSeconds; remaining > 0; remaining -= MAX_SPRING_STEP_S) {
        const dt = Math.min(remaining, MAX_SPRING_STEP_S);
        const stretch = shortestTurn(kartYaw - yaw);
        yawVelocity += (YAW_SPRING_STIFFNESS * stretch - YAW_SPRING_DAMPING * yawVelocity) * dt;
        yaw += yawVelocity * dt;
        heightVelocity +=
          (HEIGHT_SPRING_STIFFNESS * (kartY - height) - HEIGHT_SPRING_DAMPING * heightVelocity) *
          dt;
        height += heightVelocity * dt;
      }

      const spunOut = kart.spinOutS !== null;
      if (spunOut && wasSpunOut === false) {
        shakeAge = 0;
      } else {
        shakeAge = Math.min(shakeAge + dtSeconds, tuning.shakeSeconds);
      }
      wasSpunOut = spunOut;
      const [sx, sy, sz] = shake(shakeAge, tuning);

      // The pull-back follows speed alone, so a boost past top speed pulls
      // back past the tuned metre; the boost itself is felt as the kick.
      const pullBack =
        BEHIND_M +
        (Math.max(kart.speed, 0) / tuning.topSpeedMetresPerSecond) * tuning.pullBackMetres;
      // Positive is toward the kart's back; looking behind flips both, so
      // the camera sits ahead of the kart and looks behind it.
      const facing = lookingBehind ? -1 : 1;
      const cameraBack = facing * pullBack;
      const lookForward = facing * LOOK_AHEAD_M;
      return {
        // A kart with no yaw faces -Z, so behind it is +Z.
        position: [
          x + Math.sin(yaw) * cameraBack + sx,
          height + ABOVE_M + sy,
          z + Math.cos(yaw) * cameraBack + sz,
        ],
        // The target jolts with the position, so the hit shoves the whole
        // frame, and sits at the lagged height, so a hop moves the kart in
        // the frame rather than the frame with the kart.
        target: [
          x - Math.sin(kartYaw) * lookForward + sx,
          height + sy,
          z - Math.cos(kartYaw) * lookForward + sz,
        ],
        fieldOfView: fieldOfView(kart.boostS, tuning, reducedMotion),
      };
    },
  };
}

/**
 * The field of view, in degrees, for a kart with `boostS` seconds of boost
 * left: the tuned one widened by the tuned kick while the boost has more
 * than the ease left, easing back linearly to the tuned one as it runs out.
 * Off a boost, and under `reducedMotion`, the tuned one.
 */
export function fieldOfView(
  boostS: number | null,
  tuning: CameraTuning,
  reducedMotion = false,
): number {
  if (boostS === null || reducedMotion) {
    return tuning.fieldOfViewDegrees;
  }
  const ease = Math.min(Math.max(boostS, 0) / tuning.boostKickEaseSeconds, 1);
  return tuning.fieldOfViewDegrees * (1 + tuning.boostKick * ease);
}

/**
 * The shake's throw `age` seconds into it: the tuned amplitude on each axis
 * at the start, dying away linearly to nothing at the tuned length, and
 * nothing after. No randomness, so a test can hold the pose exactly.
 */
function shake(age: number, tuning: CameraTuning): readonly [number, number, number] {
  if (age >= tuning.shakeSeconds) {
    return [0, 0, 0];
  }
  const envelope = tuning.shakeAmplitudeMetres * (1 - age / tuning.shakeSeconds);
  // Cosines, so the first frame of the shake is already thrown.
  return [
    envelope * Math.cos(2 * Math.PI * SHAKE_HZ[0] * age),
    envelope * Math.cos(2 * Math.PI * SHAKE_HZ[1] * age),
    envelope * Math.cos(2 * Math.PI * SHAKE_HZ[2] * age),
  ];
}
