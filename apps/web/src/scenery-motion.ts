// The props the View moves from its own clock (ADR 0050): Presentation
// only, like the marshal's flags. Nothing here reads the bridge or the sim,
// so two clients' wheels need not agree and a stalled client's wheel stops.

import { type Object3D, Quaternion, Vector3 } from "three";

/** Milliseconds the Circuit Wheel's rim takes to turn once. */
export const WHEEL_TURN_MS = 90_000;

/**
 * The axis a rim turns about, in its node's own frame: the track builder
 * puts the axle along the node's Blender y, which the Y-up export turns
 * into its -z. A gondola hangs under its rim's node, so the same local axis
 * turns it back.
 */
const AXLE = new Vector3(0, 0, 1);

export interface SceneryMotion {
  /** Pose every moving prop for the wall clock `nowMs`, as `performance.now()` reads it. */
  update(nowMs: number): void;
}

interface Wheel {
  readonly rim: Object3D;
  readonly rimRest: Quaternion;
  readonly gondolas: readonly { readonly node: Object3D; readonly rest: Quaternion }[];
}

/**
 * The moving props in the loaded track: each `Prop.circuit-wheel.<n>.rim`
 * with its `Prop.circuit-wheel.<n>.gondola.<k>` children, found by name
 * (the loader drops the dots from the exported names). The rim turns once
 * per `WHEEL_TURN_MS` about its axle from where the export left it, and
 * each gondola is turned back by the rim's angle so it hangs level.
 */
export function sceneryMotion(track: Object3D): SceneryMotion {
  const wheels: Wheel[] = [];
  track.traverse((node) => {
    if (!/^Prop\.?circuit-wheel\.?\d+\.?rim$/.test(node.name)) return;
    const gondolas = node.children
      .filter((child) => /^Prop\.?circuit-wheel\.?\d+\.?gondola\.?\d+$/.test(child.name))
      .map((child) => ({ node: child, rest: child.quaternion.clone() }));
    wheels.push({ rim: node, rimRest: node.quaternion.clone(), gondolas });
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
    },
  };
}
