import type { KartReadout } from "./bridge";
import { shortestTurn, yawOf } from "./kart-frame";

const G_M_PER_S2 = 9.81;
const DEGREE = Math.PI / 180;
/** How far the head rolls for each g of lateral G. Visual, so not the sim's to tune. */
export const ROLL_RAD_PER_G = 4 * DEGREE;
/** The furthest the head rolls, a drift's included. */
export const MAX_ROLL_RAD = 15 * DEGREE;
/** The head is within 5% of a new lean this long after it. */
const SETTLE_S = 0.15;
const EASE_S = SETTLE_S / 3;

/** The head's sway, frame to frame. */
export interface HeadSway {
  /**
   * The head's roll about the kart's +Z, in radians, as Three rolls it:
   * positive tips the head to the kart's left. The angle to draw.
   */
  readonly roll: number;
  /** The roll the head eases toward, from the turn measured at the last tick. */
  readonly lean: number;
  /** Seconds since the readout last moved, the time the next tick's turn is measured over. */
  readonly sinceTickS: number;
}

/** A head at rest: level, leaning nowhere. */
export const HEAD_LEVEL: HeadSway = Object.freeze({ roll: 0, lean: 0, sinceTickS: 0 });

/**
 * Head sway (GLOSSARY): `sway` carried `frameSeconds` on, from `previous`,
 * the kart's readout last frame, to `current`. The head leans to the
 * outside of the turn by the lateral G, speed times the yaw rate between
 * the two readouts, and eases toward that lean. A frame that shows the same
 * tick as the last measures no turn and holds the lean, so a display faster
 * than the tick does not shake the head. Through a spin-out or a stall the
 * head is level, and it eases back into the lean once the kart drives
 * again. Presentation only.
 */
export function swayHead(
  sway: HeadSway,
  previous: KartReadout | undefined,
  current: KartReadout,
  frameSeconds: number,
): HeadSway {
  if (current.spinOutS !== null || current.stallS !== null) {
    return HEAD_LEVEL;
  }
  const sinceTickS = sway.sinceTickS + frameSeconds;
  let lean = sway.lean;
  let carried = 0;
  if (previous === undefined) {
    lean = 0;
  } else if (sameTick(previous, current)) {
    carried = sinceTickS;
  } else if (sinceTickS > 0) {
    const yawRate = shortestTurn(yawOf(current.rotation) - yawOf(previous.rotation)) / sinceTickS;
    const lateralG = (current.speed * yawRate) / G_M_PER_S2;
    // A left turn, positive yaw, tips the head right: a negative roll.
    lean = Math.min(Math.max(-lateralG * ROLL_RAD_PER_G, -MAX_ROLL_RAD), MAX_ROLL_RAD);
  }
  const ease = 1 - Math.exp(-frameSeconds / EASE_S);
  return { roll: sway.roll + (lean - sway.roll) * ease, lean, sinceTickS: carried };
}

/** Whether two readouts are one tick's: the kart has neither moved nor turned between them. */
function sameTick(a: KartReadout, b: KartReadout): boolean {
  return (
    a.position.every((v, i) => v === b.position[i]) &&
    a.rotation.every((v, i) => v === b.rotation[i])
  );
}
