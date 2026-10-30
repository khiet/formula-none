// The homing drone's tilt into its turns (ADR 0057): Presentation only, so
// its constants are the View's and not the sim's to tune.

import { shortestTurn } from "./kart-frame";

const DEGREE = Math.PI / 180;
/** The furthest a drone tilts, which it reaches turning this fast. */
export const DRONE_TILT_MAX_RAD = 30 * DEGREE;
export const DRONE_TILT_FULL_RAD_PER_S = 2;
/** The time constant the tilt eases toward the turn's with. */
export const DRONE_TILT_EASE_S = 0.12;
/**
 * A drone this frame is the one nearest it last frame within this reach,
 * further than a projectile flies in any frame the View draws.
 */
const DRONE_MATCH_REACH_M = 10;

type Position = readonly [number, number, number];

/**
 * A drone's tilt `seconds` on from `previousTilt`, as its facing yaws from
 * `previousYaw` to `yaw`: positive drops its left side, as a turn to the
 * left, where yaw grows, wants. In proportion to the turn rate up to the
 * cap and eased toward it. A drone with no `previousYaw`, just fired,
 * starts level.
 */
export function droneTilt(
  previousYaw: number | undefined,
  yaw: number,
  previousTilt: number,
  seconds: number,
): number {
  if (previousYaw === undefined) {
    return 0;
  }
  if (seconds <= 0) {
    return previousTilt;
  }
  const rate = shortestTurn(yaw - previousYaw) / seconds;
  const share = Math.min(Math.max(rate / DRONE_TILT_FULL_RAD_PER_S, -1), 1);
  const wanted = DRONE_TILT_MAX_RAD * share;
  return previousTilt + (wanted - previousTilt) * (1 - Math.exp(-seconds / DRONE_TILT_EASE_S));
}

/** A drone as the View drew it: where, its facing yaw and its tilt. */
export interface DroneFlight {
  readonly position: Position;
  readonly yaw: number;
  readonly tilt: number;
}

/**
 * Each drone in `now`, in its order, tilted `seconds` on from the drone in
 * `previous` it continues. The snapshot lists projectiles in firing order
 * with no id, so when an earlier one hits the rest move up a place; each is
 * matched instead to the nearest drone last frame, closest pairs first, so
 * a tilt stays with the drone that earned it. One with no match is new.
 */
export function flyDrones(
  previous: readonly DroneFlight[],
  now: readonly { readonly position: Position; readonly yaw: number }[],
  seconds: number,
): DroneFlight[] {
  const pairs: { readonly from: number; readonly to: number; readonly distance: number }[] = [];
  previous.forEach((before, from) => {
    now.forEach((after, to) => {
      const distance = Math.hypot(
        after.position[0] - before.position[0],
        after.position[1] - before.position[1],
        after.position[2] - before.position[2],
      );
      if (distance <= DRONE_MATCH_REACH_M) {
        pairs.push({ from, to, distance });
      }
    });
  });
  pairs.sort((a, b) => a.distance - b.distance);
  const continues = new Map<number, DroneFlight>();
  const taken = new Set<number>();
  for (const { from, to } of pairs) {
    const before = previous[from];
    if (taken.has(from) || continues.has(to) || before === undefined) {
      continue;
    }
    taken.add(from);
    continues.set(to, before);
  }
  return now.map(({ position, yaw }, to) => {
    const before = continues.get(to);
    return { position, yaw, tilt: droneTilt(before?.yaw, yaw, before?.tilt ?? 0, seconds) };
  });
}
