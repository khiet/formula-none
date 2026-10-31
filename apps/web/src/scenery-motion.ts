// The props the View moves from its own clock (ADR 0050): Presentation
// only, like the marshal's flags. Nothing here reads the bridge or the sim,
// so two clients' wheels need not agree and a stalled client's wheel stops,
// its airship hangs in the air and its jets hang mid-pass.

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
export const HELICOPTER_MAIN_ROTOR_TURNS_PER_S = 1.5;
/** Turns a second of the tail rotor. */
export const HELICOPTER_TAIL_ROTOR_TURNS_PER_S = 4.5;
/**
 * Metres of the display jets' straight pass, the layout's
 * `Prop("jets", ..., length=600.0)`, centred where the export left the
 * formation. The track tests hold the two to each other.
 */
export const JETS_PASS_M = 600;
/** Milliseconds the jets take to fly the pass: 100 m/s. */
export const JETS_PASS_MS = 6_000;
/** Milliseconds from the start of one pass to the next; the formation is hidden in between. */
export const JETS_PERIOD_MS = 30_000;

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
/** A flier's nose in its own frame: the track builder's local x, which the export keeps. */
const NOSE = new Vector3(1, 0, 0);

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

/** A formation that flies a straight pass nose first through where the export left it. */
interface Flypast {
  readonly node: Object3D;
  /** The middle of the pass, in the node's parent's frame. */
  readonly middle: Vector3;
  /** The nose's direction, in the node's parent's frame. */
  readonly nose: Vector3;
}

/** A node that spins about its own `axis` at `turnsPerS` from where the export left it. */
interface Spinner {
  readonly node: Object3D;
  readonly rest: Quaternion;
  readonly axis: Vector3;
  readonly turnsPerS: number;
}

const SPINNING: readonly { node: RegExp; axis: Vector3; turnsPerS: number }[] = [
  {
    node: /^Prop\.?helicopter\.?\d+\.?main-rotor$/,
    axis: UP,
    turnsPerS: HELICOPTER_MAIN_ROTOR_TURNS_PER_S,
  },
  {
    node: /^Prop\.?helicopter\.?\d+\.?tail-rotor$/,
    axis: AXLE,
    turnsPerS: HELICOPTER_TAIL_ROTOR_TURNS_PER_S,
  },
];

/**
 * The moving props in the loaded track, found by name (the loader drops the
 * dots from the exported names). Each `Prop.circuit-wheel.<n>.rim` with its
 * `Prop.circuit-wheel.<n>.gondola.<k>` children: the rim turns once per
 * `WHEEL_TURN_MS` about its axle from where the export left it, and each
 * gondola is turned back by the rim's angle so it hangs level. Each
 * `Prop.airship.<n>.hull` and `Prop.helicopter.<n>.body`: it flies left once
 * per its kind's period round a circle of its kind's radius, centred to its
 * left at rest, nose along the tangent, the airship level and the helicopter
 * dipped, its children riding with it. Each helicopter's
 * `.main-rotor` and `.tail-rotor` spin about their own mast and axle. Each
 * `Prop.jets.<n>.formation`: once per `JETS_PERIOD_MS` it flies nose first
 * along `JETS_PASS_M` centred where the export left it, in `JETS_PASS_MS`,
 * without turning, and is hidden for the rest of the period.
 */
export function sceneryMotion(track: Object3D): SceneryMotion {
  const wheels: CircuitWheel[] = [];
  const fliers: Flier[] = [];
  const spinners: Spinner[] = [];
  const flypasts: Flypast[] = [];
  track.traverse((node) => {
    if (/^Prop\.?circuit-wheel\.?\d+\.?rim$/.test(node.name)) {
      const gondolas = node.children
        .filter((child) => /^Prop\.?circuit-wheel\.?\d+\.?gondola\.?\d+$/.test(child.name))
        .map((child) => ({ node: child, rest: child.quaternion.clone() }));
      wheels.push({ rim: node, rimRest: node.quaternion.clone(), gondolas });
      return;
    }
    if (/^Prop\.?jets\.?\d+\.?formation$/.test(node.name)) {
      flypasts.push({
        node,
        middle: node.position.clone(),
        nose: NOSE.clone().applyQuaternion(node.quaternion),
      });
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
      const intoPeriod = nowMs % JETS_PERIOD_MS;
      const flying = intoPeriod < JETS_PASS_MS;
      const flown = (intoPeriod / JETS_PASS_MS - 0.5) * JETS_PASS_M;
      for (const { node, middle, nose } of flypasts) {
        node.visible = flying;
        if (flying) node.position.copy(nose).multiplyScalar(flown).add(middle);
      }
    },
  };
}
