// What the rain looks like from the race as it stands (ADR 0103): how far the
// clouds have gathered, how grey the sky is and how hard the streaks fall.
// Presentation only, from the wetness the sim reports, so every screen gets
// the same rain; the roof alone follows the own kart, since it comes from the
// presentation zones.

import type { Standing } from "./bridge";
import type { PresentationBlend } from "./presentation-zones";

/** The clouds gather over this share of the lap at its end. */
const CLOUD_GATHER_SHARE = 0.25;
/** The sky's grey the clouds bring on their own; the wetness brings the rest. */
const CLOUD_OVERCAST = 0.5;
/** What the sky's light is scaled by, wholly overcast. No flash ever lifts it. */
const OVERCAST_LIGHT = 0.68;
/** The RAIN plate shows through this much of the wetness's climb: five of its twenty seconds. */
const PLATE_WETNESS = 0.25;

/**
 * How far the clouds have gathered, 0 to 1, over the leader's last quarter of
 * the lap before `rainLap`: 0 when no rain is coming, and 1 from the rain's lap
 * on. A kart still on its first checkpoint counts for nothing, since one behind
 * the line on the grid stands at the end of a lap it has not driven.
 */
export function cloudCover(
  standings: readonly Standing[],
  rainLap: number | null,
  lapLength: number,
): number {
  if (rainLap === null) return 0;
  let cover = 0;
  for (const { lap, checkpoint, splineProgress } of standings) {
    if (lap >= rainLap) return 1;
    if (lap !== rainLap - 1 || checkpoint === 0) continue;
    const left = 1 - splineProgress / lapLength;
    cover = Math.max(cover, Math.min(1 - left / CLOUD_GATHER_SHARE, 1));
  }
  return cover;
}

/** The rain's look for one frame. */
export interface RainLook {
  /** How far the clouds have gathered, 0 to 1. */
  readonly clouds: number;
  /** How wet the track is, 0 to 1: the road and the spray follow this. */
  readonly wetness: number;
  /** How grey the sky is, 0 to 1. */
  readonly overcast: number;
  /** How hard the streaks fall where the own kart is, 0 to 1. */
  readonly streaks: number;
}

/**
 * The look at `wetness` under `clouds`, with `roof` how far over the own kart a
 * roof is. A roof stops the streaks and nothing else: the road is as wet under
 * it, as the grip is. In the wet the clouds are up whatever `clouds` says, as a
 * client that rejoins mid-rain never saw them gather.
 */
export function rainLook(wetness: number, clouds: number, roof: number): RainLook {
  const up = wetness > 0 ? 1 : clouds;
  return {
    clouds: up,
    wetness,
    overcast: up * CLOUD_OVERCAST + wetness * (1 - CLOUD_OVERCAST),
    streaks: wetness * (1 - roof),
  };
}

/** What the sky's light is scaled by at `overcast`. */
export function rainSkyLight(overcast: number): number {
  return 1 + overcast * (OVERCAST_LIGHT - 1);
}

/** `blend` with its light scaled under `look`'s overcast, on top of the zone's; untouched in the clear. */
export function rainBlend(blend: PresentationBlend, look: RainLook): PresentationBlend {
  return look.overcast === 0
    ? blend
    : { ...blend, light: blend.light * rainSkyLight(look.overcast) };
}

/** Whether the RAIN plate shows at `wetness`: as the rain arrives, and only then. */
export function rainPlateShowing(wetness: number): boolean {
  return wetness > 0 && wetness < PLATE_WETNESS;
}
