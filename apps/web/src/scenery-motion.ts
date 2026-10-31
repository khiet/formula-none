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
/** Milliseconds the helicopter takes to fly once round its circle. */
export const HELICOPTER_CIRCUIT_MS = 120_000;
/**
 * Metres from the circle's centre to the helicopter's nose, the layout's
 * `Prop("helicopter", ..., radius=100.0)`, found as the airship's is. The
 * track tests hold the two to each other.
 */
export const HELICOPTER_CIRCLE_M = 100;
/**
 * Degrees the helicopter's nose dips about its own left, as one flying
 * forward does. The track tests clear the roads with the body dipped by
 * this.
 */
export const HELICOPTER_DIP_DEG = 5;
/**
 * Turns a second of the main rotor: far under a real rotor's, which would
 * strobe at 60 fps into blades standing still or turning back.
 */
export const MAIN_ROTOR_TURNS_PER_S = 1.5;
/** Turns a second of the tail rotor, three times the main's, as on the real thing. */
export const TAIL_ROTOR_TURNS_PER_S = 4.5;

/**
 * The axis a rim turns about, in its node's own frame: the track builder
 * puts the axle along the node's Blender y, which the Y-up export lays
 * along its z. Turning about +z rather than -z only picks the direction of
 * turn. A gondola hangs under its rim's node, so the same local axis turns
 * it back. The tail rotor's axle is the same local axis.
 */
const AXLE = new Vector3(0, 0, 1);
/**
 * The world's up in the Y-up export, which the airship and the helicopter
 * circle about, and the main rotor's mast in its own frame.
 */
const UP = new Vector3(0, 1, 0);
/** A flier's left in its own frame: the track builder's local y, laid along -z by the export. */
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

/** How one kind of flier circles: the node it names, its circle, its period and its pitch. */
interface Circling {
  readonly node: RegExp;
  readonly radiusM: number;
  readonly periodMs: number;
  /** About the flier's own left, applied after its heading; positive puts the nose down. */
  readonly pitch: Quaternion;
}

const CIRCLING: readonly Circling[] = [
  {
    node: /^Prop\.?airship\.?\d+\.?hull$/,
    radiusM: AIRSHIP_CIRCLE_M,
    periodMs: AIRSHIP_CIRCUIT_MS,
    pitch: new Quaternion(),
  },
  {
    node: /^Prop\.?helicopter\.?\d+\.?body$/,
    radiusM: HELICOPTER_CIRCLE_M,
    periodMs: HELICOPTER_CIRCUIT_MS,
    pitch: new Quaternion().setFromAxisAngle(PORT, (HELICOPTER_DIP_DEG * Math.PI) / 180),
  },
];

interface Flier {
  readonly node: Object3D;
  readonly kind: Circling;
  /** The circle's centre, in the node's parent's frame. */
  readonly centre: Vector3;
  /** From the centre to the nose at rest. */
  readonly arm: Vector3;
  readonly rest: Quaternion;
}

/** A node that spins about its own `axis` at `turnsPerS` from where the export left it. */
interface Spinner {
  readonly node: Object3D;
  readonly rest: Quaternion;
  readonly axis: Vector3;
  readonly turnsPerS: number;
}

const SPINNING: readonly { node: RegExp; axis: Vector3; turnsPerS: number }[] = [
  { node: /^Prop\.?helicopter\.?\d+\.?main-rotor$/, axis: UP, turnsPerS: MAIN_ROTOR_TURNS_PER_S },
  { node: /^Prop\.?helicopter\.?\d+\.?tail-rotor$/, axis: AXLE, turnsPerS: TAIL_ROTOR_TURNS_PER_S },
];

/**
 * The moving props in the loaded track, found by name (the loader drops the
 * dots from the exported names). Each `Prop.circuit-wheel.<n>.rim` with its
 * `Prop.circuit-wheel.<n>.gondola.<k>` children: the rim turns once per
 * `WHEEL_TURN_MS` about its axle from where the export left it, and each
 * gondola is turned back by the rim's angle so it hangs level. Each flier,
 * `Prop.airship.<n>.hull` and `Prop.helicopter.<n>.body`: it flies once per
 * its period round the circle its radius to its left of where the export
 * left it, turning left, nose along the tangent, the airship level and the
 * helicopter dipped, its children riding with it. Each helicopter's
 * `.main-rotor` and `.tail-rotor` spin about their own mast and axle.
 */
export function sceneryMotion(track: Object3D): SceneryMotion {
  const wheels: CircuitWheel[] = [];
  const fliers: Flier[] = [];
  const spinners: Spinner[] = [];
  track.traverse((node) => {
    if (/^Prop\.?circuit-wheel\.?\d+\.?rim$/.test(node.name)) {
      const gondolas = node.children
        .filter((child) => /^Prop\.?circuit-wheel\.?\d+\.?gondola\.?\d+$/.test(child.name))
        .map((child) => ({ node: child, rest: child.quaternion.clone() }));
      wheels.push({ rim: node, rimRest: node.quaternion.clone(), gondolas });
      return;
    }
    const kind = CIRCLING.find(({ node: name }) => name.test(node.name));
    if (kind !== undefined) {
      const rest = node.quaternion.clone();
      const centre = PORT.clone()
        .applyQuaternion(rest)
        .multiplyScalar(kind.radiusM)
        .add(node.position);
      fliers.push({ node, kind, centre, arm: node.position.clone().sub(centre), rest });
      return;
    }
    const spinning = SPINNING.find(({ node: name }) => name.test(node.name));
    if (spinning !== undefined) {
      spinners.push({
        node,
        rest: node.quaternion.clone(),
        axis: spinning.axis,
        turnsPerS: spinning.turnsPerS,
      });
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
      for (const { node, kind, centre, arm, rest } of fliers) {
        const flown = ((nowMs % kind.periodMs) / kind.periodMs) * Math.PI * 2;
        spin.setFromAxisAngle(UP, flown);
        node.position.copy(arm).applyQuaternion(spin).add(centre);
        node.quaternion.copy(spin).multiply(rest).multiply(kind.pitch);
      }
      for (const { node, rest, axis, turnsPerS } of spinners) {
        // Taken modulo a turn's milliseconds so the angle keeps its precision on a long clock.
        const turnMs = 1000 / turnsPerS;
        spin.setFromAxisAngle(axis, ((nowMs % turnMs) / turnMs) * Math.PI * 2);
        node.quaternion.copy(rest).multiply(spin);
      }
    },
  };
}
