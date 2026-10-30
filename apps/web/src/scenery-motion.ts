// The props the View moves from its own clock (ADR 0050): Presentation
// only, like the marshal's flags. Nothing here reads the bridge or the sim,
// so two clients' wheels need not agree and a stalled client's wheel stops
// and its airship hangs in the air.

import { type Object3D, Quaternion, Vector3 } from "three";

/** Milliseconds the Circuit Wheel's rim takes to turn once. */
export const WHEEL_TURN_MS = 90_000;
/** Milliseconds the airship takes to fly once round its circle. */
export const AIRSHIP_CIRCUIT_MS = 240_000;
/**
 * Metres from the circle's centre to the airship's nose, the layout's
 * `Prop("airship", ..., radius=60.0)`: the export carries the hull at rest
 * on the circle with its nose along it and the centre to its left, so the
 * radius is all the View needs to find the centre.
 */
export const AIRSHIP_CIRCLE_M = 60;

/**
 * The axis a rim turns about, in its node's own frame: the track builder
 * puts the axle along the node's Blender y, which the Y-up export lays
 * along its z. Turning about +z rather than -z only picks the direction of
 * turn. A gondola hangs under its rim's node, so the same local axis turns
 * it back.
 */
const AXLE = new Vector3(0, 0, 1);
/** The world's up in the Y-up export, which the airship circles about. */
const UP = new Vector3(0, 1, 0);
/** The hull's left in its own frame: the track builder's local y, laid along -z by the export. */
const PORT = new Vector3(0, 0, -1);

export interface SceneryMotion {
  /** Pose every moving prop for the wall clock `nowMs`, as `performance.now()` reads it. */
  update(nowMs: number): void;
}

interface CircuitWheel {
  readonly rim: Object3D;
  readonly rimRest: Quaternion;
  readonly gondolas: readonly { readonly node: Object3D; readonly rest: Quaternion }[];
}

interface Airship {
  readonly hull: Object3D;
  /** The circle's centre, in the hull's parent's frame. */
  readonly centre: Vector3;
  /** From the centre to the nose at rest. */
  readonly arm: Vector3;
  readonly rest: Quaternion;
}

/**
 * The moving props in the loaded track, found by name (the loader drops the
 * dots from the exported names). Each `Prop.circuit-wheel.<n>.rim` with its
 * `Prop.circuit-wheel.<n>.gondola.<k>` children: the rim turns once per
 * `WHEEL_TURN_MS` about its axle from where the export left it, and each
 * gondola is turned back by the rim's angle so it hangs level. Each
 * `Prop.airship.<n>.hull`: it flies once per `AIRSHIP_CIRCUIT_MS` round the
 * circle `AIRSHIP_CIRCLE_M` to its left of where the export left it, level,
 * nose along the tangent, its name riding with it as its child.
 */
export function sceneryMotion(track: Object3D): SceneryMotion {
  const wheels: CircuitWheel[] = [];
  const airships: Airship[] = [];
  track.traverse((node) => {
    if (/^Prop\.?circuit-wheel\.?\d+\.?rim$/.test(node.name)) {
      const gondolas = node.children
        .filter((child) => /^Prop\.?circuit-wheel\.?\d+\.?gondola\.?\d+$/.test(child.name))
        .map((child) => ({ node: child, rest: child.quaternion.clone() }));
      wheels.push({ rim: node, rimRest: node.quaternion.clone(), gondolas });
    } else if (/^Prop\.?airship\.?\d+\.?hull$/.test(node.name)) {
      const rest = node.quaternion.clone();
      const centre = PORT.clone()
        .applyQuaternion(rest)
        .multiplyScalar(AIRSHIP_CIRCLE_M)
        .add(node.position);
      airships.push({ hull: node, centre, arm: node.position.clone().sub(centre), rest });
    }
  });
  const spin = new Quaternion();
  return {
    update(nowMs) {
      const angle = ((nowMs % WHEEL_TURN_MS) / WHEEL_TURN_MS) * Math.PI * 2;
      for (const wheel of wheels) {
        spin.setFromAxisAngle(AXLE, angle);
        wheel.rim.quaternion.copy(wheel.rimRest).multiply(spin);
        spin.setFromAxisAngle(AXLE, -angle);
        for (const { node, rest } of wheel.gondolas) {
          node.quaternion.copy(rest).multiply(spin);
        }
      }
      // Turning left about the world's up: the Blender builder's counter-clockwise, which
      // the Y-up export keeps, since it only rotates the frame.
      const flown = ((nowMs % AIRSHIP_CIRCUIT_MS) / AIRSHIP_CIRCUIT_MS) * Math.PI * 2;
      spin.setFromAxisAngle(UP, flown);
      for (const { hull, centre, arm, rest } of airships) {
        hull.position.copy(arm).applyQuaternion(spin).add(centre);
        hull.quaternion.copy(spin).multiply(rest);
      }
    },
  };
}
