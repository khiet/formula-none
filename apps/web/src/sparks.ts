import type { Drift, MiniTurbo } from "./bridge";

/** The spark tell at a kart's wheels: shown while it drifts, coloured by the mini-turbo tier. */
export interface SparkTell {
  /** A Three.js colour hex. */
  readonly colour: number;
}

/** Spark colour by tier; the tell changes colour as blue and then orange charge. */
const SPARK_COLOURS: Readonly<Record<MiniTurbo, number>> = {
  none: 0xffffff,
  blue: 0x3399ff,
  orange: 0xff8800,
};

/**
 * The Feedback for a drift (GLOSSARY): placeholder sparks at the wheels
 * while the kart drifts, in the colour of the tier charged so far, and
 * nothing when it is not drifting. Presentation only; reads state.
 */
export function sparkTell(drift: Drift | null): SparkTell | null {
  return drift === null ? null : { colour: SPARK_COLOURS[drift.miniTurbo] };
}
