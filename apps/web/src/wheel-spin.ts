import { Box3, type Object3D } from "three";

/** The wheel nodes in the exported kart, each built about its axle and unrotated. */
export const WHEELS = [
  "wheel-front-left",
  "wheel-front-right",
  "wheel-back-left",
  "wheel-back-right",
] as const;

/**
 * The most a wheel turns in one frame: under half the 120 degree repeat of
 * its three spokes, so a fast wheel always reads as turning forwards. A
 * per-frame angle, not a rate, since strobing depends on the frame rate.
 */
const WHEEL_SPIN_CAP_RAD = (50 * Math.PI) / 180;

const TURN_RAD = 2 * Math.PI;

/**
 * A wheel's angle after `dtSeconds` at `speed` metres per second, from
 * `angle`: it grows as the kart drives forwards, so a kart backing up turns
 * its wheels backwards. Kept within a turn either way.
 */
export function wheelSpin(angle: number, speed: number, radius: number, dtSeconds: number): number {
  const step = (speed / radius) * dtSeconds;
  const capped = Math.max(-WHEEL_SPIN_CAP_RAD, Math.min(WHEEL_SPIN_CAP_RAD, step));
  return (angle + capped) % TURN_RAD;
}

export interface KartWheels {
  /** Turn every wheel by this frame's share at the kart's `speed`. */
  turn(speed: number, dtSeconds: number): void;
}

/**
 * The wheels of `model`, a dressed kart not yet fitted, which is drawn at
 * `scale`. Each wheel's radius is half its height, measured from
 * its bounds, so it follows whatever the exporter built.
 */
export function kartWheels(model: Object3D, scale: number): KartWheels {
  const wheels = WHEELS.flatMap((name) => {
    const node = model.getObjectByName(name);
    if (node === undefined) {
      return [];
    }
    const box = new Box3().setFromObject(node);
    return [{ node, radius: ((box.max.y - box.min.y) / 2) * scale, angle: 0 }];
  });
  return {
    turn(speed, dtSeconds) {
      for (const wheel of wheels) {
        wheel.angle = wheelSpin(wheel.angle, speed, wheel.radius, dtSeconds);
        // The axle is the model's X and its front is -Z: driving forwards
        // carries the top of the wheel towards -Z, a negative turn about X.
        wheel.node.rotation.x = -wheel.angle;
      }
    },
  };
}
