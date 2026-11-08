/** Seconds of the driver's flourish; about a flight off Bell Deer's ramp. Visual, so not the sim's to tune. */
export const FLOURISH_S = 0.35;

/** The driver's flourish, frame to frame. */
export interface Flourish {
  /** Seconds since the trick was earned; null with no flourish under way. */
  readonly seconds: number | null;
  /** Whether the readout last frame held a trick, so a new one starts it once. */
  readonly tricking: boolean;
}

/** A driver at rest. */
export const NO_FLOURISH: Flourish = Object.freeze({ seconds: null, tricking: false });

/**
 * The flourish (GLOSSARY: trick) carried `frameSeconds` on: it starts as the
 * readout's trick is earned and runs its whole length by the clock, so a
 * trick earned late in a flight still ends with the head facing forward.
 * Presentation only.
 */
export function flourishAfter(flourish: Flourish, trick: boolean, frameSeconds: number): Flourish {
  if (trick && !flourish.tricking) {
    return { seconds: 0, tricking: true };
  }
  const seconds = flourish.seconds === null ? null : flourish.seconds + frameSeconds;
  return { seconds: seconds !== null && seconds < FLOURISH_S ? seconds : null, tricking: trick };
}

/** How far round the driver's head has turned about the neck, in radians: one whole turn, eased out. */
export function flourishTurn({ seconds }: Flourish): number {
  if (seconds === null) {
    return 0;
  }
  const left = 1 - seconds / FLOURISH_S;
  return 2 * Math.PI * (1 - left * left);
}
