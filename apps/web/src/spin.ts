const SPIN_OUT_TURNS = 2;

/**
 * The Feedback for a spin-out (GLOSSARY): how far round a spun-out kart has
 * turned about its axis, in radians, with `left` seconds of a spin-out of
 * `total` to go. The hit spins the kart fastest at once and the spin bleeds
 * away over whole turns across the whole spin-out, so the kart faces where
 * the sim has it again as the spin-out ends. Presentation only.
 */
export function spinOutAngle(left: number, total: number): number {
  const progress = Math.min(Math.max(total - left, 0), total) / total;
  // Quadratic ease out: the rate of turn falls linearly to nothing.
  const eased = 1 - (1 - progress) * (1 - progress);
  return 2 * Math.PI * SPIN_OUT_TURNS * eased;
}
