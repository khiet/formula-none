import type { KartReadout } from "./bridge";
import { shortestTurn, yawOf } from "./kart-frame";

const G_M_PER_S2 = 9.81;
const DEGREE = Math.PI / 180;
/** How far the head rolls for each g of lateral G. Visual, so not the sim's to tune. */
export const ROLL_RAD_PER_G = 4 * DEGREE;
/** The furthest the head rolls, a drift's included. */
export const MAX_ROLL_RAD = 15 * DEGREE;
/** The head is within 5% of a new lean this long after the turn changes. */
const SETTLE_S = 0.15;
/**
 * The time constant of each of the two lags between the turn and the head:
 * two equal first-order lags come within 5% of a step in 4.74 of them.
 */
const LAG_S = SETTLE_S / 4.74;

/** The head's sway, frame to frame. */
export interface HeadSway {
  /**
   * The head's roll about the kart's +Z, in radians, as Three rolls it:
   * positive tips the head to the kart's left. The angle to draw.
   */
  readonly roll: number;
  /** The lateral G the head leans by, lagged; positive in a left turn. */
  readonly lateralG: number;
}

/** A head at rest: level, leaning nowhere. */
export const HEAD_LEVEL: HeadSway = Object.freeze({ roll: 0, lateralG: 0 });

/**
 * Head sway (GLOSSARY): `sway` carried `frameSeconds` on, from `previous`,
 * the kart's readout last frame, to `current`. The head leans to the
 * outside of the turn by the lateral G, speed times the yaw rate between
 * the two readouts, and settles on a new lean in about 0.15 s. The G is
 * lagged over frame time, so a frame that repeats the last tick leaves the
 * lean about the same at any display rate. Through a spin-out or a stall
 * the head is level, and it eases back into the lean once the kart drives
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
  if (previous === undefined || frameSeconds <= 0) {
    return sway;
  }
  const yawRate = shortestTurn(yawOf(current.rotation) - yawOf(previous.rotation)) / frameSeconds;
  const follow = 1 - Math.exp(-frameSeconds / LAG_S);
  const lateralG =
    sway.lateralG + ((current.speed * yawRate) / G_M_PER_S2 - sway.lateralG) * follow;
  // A left turn, positive yaw, tips the head right: a negative roll.
  const lean = Math.min(Math.max(-lateralG * ROLL_RAD_PER_G, -MAX_ROLL_RAD), MAX_ROLL_RAD);
  return { roll: sway.roll + (lean - sway.roll) * follow, lateralG };
}
