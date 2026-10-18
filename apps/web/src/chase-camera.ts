import type { KartTransform } from "./bridge";

/** Where the Race View's camera sits and what it looks at, in world metres. */
export interface CameraPose {
  readonly position: readonly [number, number, number];
  readonly target: readonly [number, number, number];
}

export interface ChaseCamera {
  /** The pose for this frame, `dtSeconds` after the last one, following `kart`. */
  update(kart: KartTransform, dtSeconds: number): CameraPose;
}

const BEHIND_M = 6;
const ABOVE_M = 2.5;
const LOOK_AHEAD_M = 4;
/**
 * A spring on the camera's yaw toward the kart's. Critically damped, so it
 * settles behind the kart without swinging past it.
 */
const YAW_SPRING_STIFFNESS = 20;
const YAW_SPRING_DAMPING = 2 * Math.sqrt(YAW_SPRING_STIFFNESS);
/** The spring integrates in steps no longer than this, so a stalled frame cannot fling it. */
const MAX_SPRING_STEP_S = 1 / 60;

/**
 * A third-person camera behind and above the local kart. Its yaw lags the
 * kart's through a damped spring, so a corner swings the kart across the
 * frame before the camera follows, and it looks a little ahead of the kart
 * along the kart's own heading. Presentation only: it reads transforms the
 * sim reported and never touches the sim.
 */
export function createChaseCamera(): ChaseCamera {
  let yaw: number | undefined;
  let yawVelocity = 0;
  return {
    update(kart, dtSeconds) {
      const kartYaw = yawOf(kart.rotation);
      if (yaw === undefined) {
        yaw = kartYaw;
      }
      for (let remaining = dtSeconds; remaining > 0; remaining -= MAX_SPRING_STEP_S) {
        const dt = Math.min(remaining, MAX_SPRING_STEP_S);
        const stretch = shortestTurn(kartYaw - yaw);
        yawVelocity += (YAW_SPRING_STIFFNESS * stretch - YAW_SPRING_DAMPING * yawVelocity) * dt;
        yaw += yawVelocity * dt;
      }
      const [x, y, z] = kart.position;
      return {
        // A kart with no yaw faces -Z, so behind it is +Z.
        position: [x + Math.sin(yaw) * BEHIND_M, y + ABOVE_M, z + Math.cos(yaw) * BEHIND_M],
        target: [x - Math.sin(kartYaw) * LOOK_AHEAD_M, y, z - Math.cos(kartYaw) * LOOK_AHEAD_M],
      };
    },
  };
}

/** Yaw about +Y from a unit quaternion `[x, y, z, w]`. */
function yawOf([x, y, z, w]: readonly [number, number, number, number]): number {
  return Math.atan2(2 * (w * y + x * z), 1 - 2 * (y * y + z * z));
}

/** `angle` wrapped to [-pi, pi], so the spring never goes the long way round. */
function shortestTurn(angle: number): number {
  return Math.atan2(Math.sin(angle), Math.cos(angle));
}
