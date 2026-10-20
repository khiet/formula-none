/** A spun-out kart turns this many times over its spin-out. */
const SPIN_OUT_TURNS = 2;

/**
 * The Feedback for a spin-out (GLOSSARY): how far round a spun-out kart has
 * turned about its axis, in radians, with `left` seconds of a spin-out of
 * `total` to go. The tuned turns run over the whole spin-out, so the kart
 * faces where the sim has it again as the spin-out ends, and nothing
 * between the sim and the View needs correcting. Presentation only.
 */
export function spinOutAngle(left: number, total: number): number {
  const elapsed = Math.min(Math.max(total - left, 0), total);
  return (2 * Math.PI * SPIN_OUT_TURNS * elapsed) / total;
}
