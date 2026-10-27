// The flame at a kart's exhaust while it boosts: the release's tell, for
// every boost the sim reports, a mini-turbo, a rocket start, a boost pad or
// an item. The readout tells boost seconds and not their source, so the
// source is worked out from two readings, as the sound cues do (ADR 0028).
// Presentation only, decided from the readouts the bridge reports.

import type { KartReadout, KartSize, MiniTurbo } from "./bridge";
import type { Start } from "./generated/protocol";
import { rotate, toWorld, type Vec3 } from "./kart-frame";

/** The tier a flame burns in, the spark tiers' colours. */
export type FlameTier = Exclude<MiniTurbo, "none">;

/** One kart's flame: whether it burns, in which tier, how strongly and where. */
export interface Flame {
  /** False off a boost, and for a slot the sim did not report. */
  readonly visible: boolean;
  readonly tier: FlameTier;
  /** 1 as the boost begins, shrinking toward 0 as it runs out. */
  readonly strength: number;
  /** The exhaust, at the middle of the kart's tail, in world metres. */
  readonly position: Readonly<Vec3>;
  /** Straight back out of the kart, unit length, in the world's frame. */
  readonly back: Readonly<Vec3>;
}

export interface ExhaustFlames {
  /** One flame per slot, rewritten by `update`. */
  readonly flames: readonly Flame[];
  /**
   * Burn each slot's flame from its readout and the one `update` saw
   * last; `countingDown` while the race counts down, when a rocket start's
   * hold is charging. Slots past `karts` go out.
   */
  update(karts: readonly KartReadout[], countingDown: boolean): void;
}

/** A flame is full size at a boost's start and half size as it runs out. */
const FLAME_LEAST_SIZE = 0.5;
/** Over this last share of the boost the flame fades to nothing. */
const FLAME_FADE_SHARE = 0.25;

interface MutableFlame {
  visible: boolean;
  tier: FlameTier;
  strength: number;
  position: Vec3;
  back: Vec3;
}

/** What one kart's last reading leaves for the next to compare against. */
interface Previous {
  boostS: number | null;
  /** The tier the drift under way had charged; null off a drift. */
  driftTier: MiniTurbo | null;
  /** The rocket start's charge while the countdown ran; null from Go. */
  charge: Start | null;
  /** Boost seconds when the boost burning began or last lengthened. */
  boostLengthS: number;
}

/**
 * A flame per slot. The boost an orange drift's release gives, and a weak
 * rocket start's at Go, burns orange; every other boost, a red release, a
 * full rocket start, a pad or an item, burns red. A boost a release
 * replaces, or another source lengthens, takes the new source's tier. The flame's strength is the share of the boost left: full size
 * at the start, shrinking to half and fading out over its last quarter.
 */
export function createExhaustFlames(kartCount: number, size: KartSize): ExhaustFlames {
  const flames: MutableFlame[] = Array.from({ length: kartCount }, () => ({
    visible: false,
    tier: "red",
    strength: 0,
    position: [0, 0, 0],
    back: [0, 0, 1],
  }));
  const previous: (Previous | undefined)[] = Array.from({ length: kartCount }, () => undefined);
  return {
    flames,
    update(karts, countingDown) {
      flames.forEach((flame, slot) => {
        const kart = karts[slot];
        if (kart === undefined) {
          flame.visible = false;
          previous[slot] = undefined;
          return;
        }
        const before = previous[slot];
        const driftTier = kart.drift !== null ? kart.miniTurbo : null;
        const charge = countingDown ? kart.start : null;
        const { boostS } = kart;
        let boostLengthS = before?.boostLengthS ?? 0;
        // A release replaces whatever boost was running, a longer one
        // included, so it is read from the drift ending, not the boost growing.
        const released =
          boostS !== null &&
          (before?.driftTier === "orange" || before?.driftTier === "red") &&
          driftTier === null;
        // The hold can cross a tier on the Go tick: the verdict, not the
        // last charge, says which boost the start earned.
        const rocketStarted = boostS !== null && before?.charge != null && charge === null;
        const grew = boostS !== null && (before?.boostS == null || boostS > before.boostS);
        if (boostS !== null && (released || rocketStarted || grew)) {
          const orange = released
            ? before?.driftTier === "orange"
            : rocketStarted && kart.start === "weakBoost";
          flame.tier = orange ? "orange" : "red";
          boostLengthS = boostS;
        }
        previous[slot] = { boostS, driftTier, charge, boostLengthS };
        flame.visible = boostS !== null;
        if (boostS === null) {
          flame.strength = 0;
          return;
        }
        const share = boostLengthS > 0 ? Math.min(Math.max(boostS / boostLengthS, 0), 1) : 0;
        flame.strength =
          Math.min(share / FLAME_FADE_SHARE, 1) *
          (FLAME_LEAST_SIZE + (1 - FLAME_LEAST_SIZE) * share);
        toWorld(kart, 0, 0, size.length / 2, flame.position);
        rotate(kart.rotation, 0, 0, 1, flame.back);
      });
    },
  };
}
