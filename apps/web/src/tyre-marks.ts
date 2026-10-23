import type { KartSize, KartTransform } from "./bridge";
import { rearWheel, type Side, type Vec3 } from "./kart-frame";

/** One stretch of rubber on the road: a tyre's print between two frames. */
export interface TyreMark {
  /** Where the tyre was and where it is, on the floor of the kart's box, in world metres. */
  readonly from: Readonly<Vec3>;
  readonly to: Readonly<Vec3>;
  /** How dark the mark still is: 1 fresh, falling to 0 at the end of its life. */
  readonly strength: number;
}

export interface TyreMarks {
  /**
   * Lay a mark under each rear wheel of every kart that was drifting last
   * frame and still is, from where its last mark ended to where the wheel
   * is once that is a mark's length away, and age every mark by
   * `dtSeconds`.
   */
  update(transforms: readonly KartTransform[], dtSeconds: number): void;
  /**
   * Every mark still showing, oldest first within one lap of the store.
   * `mark` is reused between calls: copy what is kept.
   */
  forEach(visit: (mark: TyreMark, slot: number) => void): void;
  /** How many marks the store holds at once; `forEach` never passes a slot past it. */
  readonly capacity: number;
}

export interface TyreMarkOptions {
  /** Seconds a mark takes to fade away. */
  readonly lifetimeSeconds?: number;
  /** Marks kept per wheel; past that the oldest in the store goes. */
  readonly capacityPerWheel?: number;
}

/**
 * A mark is laid once the wheel is this far from the last one, whatever
 * the frame rate, so a trail is the same at 120 Hz as at 60: short enough
 * to follow a drift's arc, long enough that top speed lays a hundred a
 * second.
 */
const MARK_LENGTH_M = 0.3;
const DEFAULT_LIFETIME_S = 4;
/** A lifetime of marks at top speed; a boost past it replaces the oldest a little early. */
const DEFAULT_CAPACITY_PER_WHEEL = 400;
const SIDES: readonly Side[] = [-1, 1];

/**
 * The Feedback for a drift (GLOSSARY): rubber laid under the rear wheels for
 * as long as the sim reports the kart drifting, fading over its life. The
 * trail breaks when the drift ends, so two drifts never join. Marks are
 * laid by distance travelled, not per frame, and a fixed ring of them is
 * overwritten oldest first with nothing allocated per frame, so eight karts
 * drifting at once lay marks at a steady cost at any frame rate.
 * Presentation only: reads transforms, never input.
 */
export function createTyreMarks(
  kartCount: number,
  size: KartSize,
  options: TyreMarkOptions = {},
): TyreMarks {
  const lifetime = options.lifetimeSeconds ?? DEFAULT_LIFETIME_S;
  const capacity =
    kartCount * SIDES.length * (options.capacityPerWheel ?? DEFAULT_CAPACITY_PER_WHEEL);
  // Struct of arrays: six floats of ends per mark and its age; a slot is
  // live while its age is under the lifetime.
  const ends = new Float32Array(capacity * 6);
  const ages = new Float32Array(capacity).fill(lifetime);
  /** The slot the next mark goes in: the oldest once the store has filled. */
  let head = 0;
  /** Where each wheel's trail ends while it drifts, or NaN when it does not. */
  const lastWheel = new Float32Array(kartCount * SIDES.length * 3).fill(Number.NaN);
  const wheel: Vec3 = [0, 0, 0];
  const mark = { from: [0, 0, 0] as Vec3, to: [0, 0, 0] as Vec3, strength: 0 };

  return {
    capacity,
    update(transforms, dtSeconds) {
      for (let slot = 0; slot < capacity; slot++) {
        ages[slot] = Math.min((ages[slot] ?? lifetime) + dtSeconds, lifetime);
      }
      for (let kart = 0; kart < kartCount; kart++) {
        const transform = transforms[kart];
        SIDES.forEach((side, index) => {
          const at = (kart * SIDES.length + index) * 3;
          const lastX = lastWheel[at] ?? Number.NaN;
          if (transform === undefined || transform.drift === null) {
            lastWheel[at] = Number.NaN;
            return;
          }
          rearWheel(transform, side, size, wheel);
          if (Number.isNaN(lastX)) {
            lastWheel.set(wheel, at);
            return;
          }
          const lastY = lastWheel[at + 1] ?? 0;
          const lastZ = lastWheel[at + 2] ?? 0;
          if (Math.hypot(wheel[0] - lastX, wheel[1] - lastY, wheel[2] - lastZ) < MARK_LENGTH_M) {
            return;
          }
          ends.set([lastX, lastY, lastZ, wheel[0], wheel[1], wheel[2]], head * 6);
          ages[head] = 0;
          head = (head + 1) % capacity;
          lastWheel.set(wheel, at);
        });
      }
    },
    forEach(visit) {
      for (let step = 0; step < capacity; step++) {
        const slot = (head + step) % capacity;
        const age = ages[slot] ?? lifetime;
        if (age >= lifetime) {
          continue;
        }
        const at = slot * 6;
        mark.from[0] = ends[at] ?? 0;
        mark.from[1] = ends[at + 1] ?? 0;
        mark.from[2] = ends[at + 2] ?? 0;
        mark.to[0] = ends[at + 3] ?? 0;
        mark.to[1] = ends[at + 4] ?? 0;
        mark.to[2] = ends[at + 5] ?? 0;
        mark.strength = 1 - age / lifetime;
        visit(mark, slot);
      }
    },
  };
}
