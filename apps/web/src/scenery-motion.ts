// The props the View moves from its own clock (ADR 0050): Presentation
// only, like the marshal's flags. Nothing here reads the bridge or the sim,
// so two clients' wheels need not agree and a stalled client's wheel stops,
// its airship hangs in the air and its jets hang mid-pass. The jets' pass
// alone is cued, by the own kart's spline progress (ADR 0099), so each
// client's jets meet its own kart.

import { type Object3D, Quaternion, Vector3 } from "three";
import type { TrackName } from "./track";

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
/**
 * Milliseconds from the cue to the formation over the middle of its pass:
 * a kart at 25 to 30 m/s out of Chapel runs the 109 m to the Hangar
 * Straight's middle in 3.6 to 4.4 s, so the formation is within the
 * hangars' 110 m row as it gets there. A kart off that pace, boosted off
 * the straight's pad or on worn tyres, meets it off the row's middle. The
 * pass starts half a pass sooner, a second after the cue, as the kart
 * leaves Chapel.
 */
export const JETS_OVERHEAD_MS = 4_000;
/** Milliseconds from a cue to its pass starting and to its pass ending. */
const JETS_PASS_FROM_MS = JETS_OVERHEAD_MS - JETS_PASS_MS / 2;
const JETS_PASS_TO_MS = JETS_OVERHEAD_MS + JETS_PASS_MS / 2;
/**
 * The start of Goldstone's `chapel`, in metres of spline progress from the
 * line, the layout's (`assets/tracks/trackgen/layouts/goldstone.py`); the
 * track tests hold the two to each other.
 */
export const GOLDSTONE_CHAPEL_M = 816.79;
/** Milliseconds from one wave breaking over the quay to the next. */
export const SPRAY_PERIOD_MS = 8_000;
/** Milliseconds a burst of spray takes to rise and fall; it is hidden the rest of the period. */
export const SPRAY_BURST_MS = 1_200;
/**
 * Milliseconds from one burst's start to the next along the quay, in the
 * export's numbering, the lap's order, so the wave runs the way the karts do.
 */
export const SPRAY_STAGGER_MS = 100;
/**
 * How much wider than at rest a burst has spread by the end of its fall. The
 * track export places bursts so their full spread stays over the quay, and
 * the track tests hold its `SPRAY_SPREAD` to this.
 */
export const SPRAY_SPREAD = 0.5;

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
  /**
   * Pose every moving prop for the wall clock `nowMs`, as `performance.now()`
   * reads it, flying the jets' pass from `flypastCuedMs`, the wall clock of
   * the last cue `flypastTimer` reported; null while none has come, which
   * keeps the jets hidden.
   */
  update(nowMs: number, flypastCuedMs?: number | null): void;
}

/** The own kart's spline progress that cues `track`'s flypast, or null on a course with none. */
export function flypastCueOf(track: TrackName): number | null {
  return track === "goldstone" ? GOLDSTONE_CHAPEL_M : null;
}

export interface FlypastTimer {
  /**
   * Read the own kart at `splineProgress` metres on the wall clock `nowMs`,
   * and give the wall clock of the latest cue, null before the first. A cue
   * comes as the kart moves forward onto or over the cue's progress, and
   * not while the pass it set is still to fly or flying, so a kart spun back
   * over the cue never restarts it.
   */
  update(nowMs: number, splineProgress: number): number | null;
}

/**
 * Cues the jets from the own kart's progress on a lap of `lapM` metres,
 * crossing `cueM`; never with a null `cueM`. A step of half a lap or more
 * is a lap's wrap or a reset, never a crossing.
 */
export function flypastTimer(cueM: number | null, lapM: number): FlypastTimer {
  let last: number | null = null;
  let cuedMs: number | null = null;
  return {
    update(nowMs, splineProgress) {
      const before = last;
      last = splineProgress;
      if (cueM === null || before === null) return cuedMs;
      const step = wrap(splineProgress - before, lapM);
      const toCue = wrap(cueM - before, lapM);
      const crossed = step < lapM / 2 && toCue > 0 && toCue <= step;
      const inFlight = cuedMs !== null && nowMs - cuedMs < JETS_PASS_TO_MS;
      if (crossed && !inFlight) cuedMs = nowMs;
      return cuedMs;
    },
  };
}

function wrap(metres: number, length: number): number {
  const wrapped = metres % length;
  return wrapped < 0 ? wrapped + length : wrapped;
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

/** A burst of spray on the quay, the `index`th from zero along it. */
interface Burst {
  readonly node: Object3D;
  readonly index: number;
  readonly rest: Vector3;
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
 * `Prop.jets.<n>.formation`: after each cue it flies nose first along
 * `JETS_PASS_M` centred where the export left it, in `JETS_PASS_MS`, over
 * its middle `JETS_OVERHEAD_MS` after the cue, without turning, and is
 * hidden the rest of the time. Each
 * `Prop.harbour.<n>.spray.<k>`: once per `SPRAY_PERIOD_MS`, starting
 * `SPRAY_STAGGER_MS` after burst `k - 1`, it rises from its foot to the
 * height the export left it and falls again in `SPRAY_BURST_MS`, spreading
 * as it falls, and is hidden for the rest of the period.
 */
export function sceneryMotion(track: Object3D): SceneryMotion {
  const wheels: CircuitWheel[] = [];
  const fliers: Flier[] = [];
  const spinners: Spinner[] = [];
  const flypasts: Flypast[] = [];
  const bursts: Burst[] = [];
  track.traverse((node) => {
    const spray = /^Prop\.?harbour\.?\d+\.?spray\.?(\d+)$/.exec(node.name);
    if (spray !== null) {
      bursts.push({ node, index: Number(spray[1]) - 1, rest: node.scale.clone() });
      return;
    }
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
    update(nowMs, flypastCuedMs = null) {
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
      const intoPass = flypastCuedMs === null ? -1 : nowMs - (flypastCuedMs + JETS_PASS_FROM_MS);
      const flying = intoPass >= 0 && intoPass < JETS_PASS_MS;
      const flown = (intoPass / JETS_PASS_MS - 0.5) * JETS_PASS_M;
      for (const { node, middle, nose } of flypasts) {
        node.visible = flying;
        if (flying) node.position.copy(nose).multiplyScalar(flown).add(middle);
      }
      for (const { node, index, rest } of bursts) {
        const into =
          (((nowMs - index * SPRAY_STAGGER_MS) % SPRAY_PERIOD_MS) + SPRAY_PERIOD_MS) %
          SPRAY_PERIOD_MS;
        node.visible = into < SPRAY_BURST_MS;
        if (node.visible) {
          const share = into / SPRAY_BURST_MS;
          // Never quite flat: a zero scale leaves the renderer no normals to light it by.
          const rise = Math.max(Math.sin(Math.PI * share), 0.01);
          const spread = 1 + SPRAY_SPREAD * share;
          node.scale.set(rest.x * spread, rest.y * rise, rest.z * spread);
        }
      }
    },
  };
}
