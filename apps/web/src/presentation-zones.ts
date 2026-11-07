// Presentation zones (ADR 0098): stretches of a course, by spline progress,
// where the View's light and the audio's ambience and crowd change. Keyed
// by progress and never by position, so where a course crosses itself the
// leg the kart is on decides. Presentation only: the sim never reads a zone,
// so a zone changes neither grip nor timing.

import type { TrackName } from "./track";

/** The ambient beds a zone can bring up, each a loop the audio levels by the blend. */
export type AmbienceBed = "harbour";

/**
 * One zone: it eases in from `from` to `fullFrom`, holds its whole look to
 * `fullTo` and eases out by `to`, each in metres of spline progress from the
 * start line. A zone that runs over the line has `to` short of `from`.
 */
export interface PresentationZone {
  readonly from: number;
  readonly fullFrom: number;
  readonly fullTo: number;
  readonly to: number;
  /** What the zone scales the sky's light by when wholly in; 1 leaves it. */
  readonly light: number;
  /** How much of the engine's echo the zone sends, 0 to 1. */
  readonly echo: number;
  /** Overhead lamps the kart passes under, from the first's progress, a lamp every `spacing` metres; null for none. */
  readonly lamps: { readonly firstAt: number; readonly spacing: number } | null;
  /** Each bed's level when wholly in, 0 to 1. */
  readonly ambience: Readonly<Record<AmbienceBed, number>>;
  /** The crowd's level when wholly in, 0 to 1. */
  readonly crowd: number;
}

/** What the View and the audio are given for one frame of the own kart. */
export interface PresentationBlend {
  /** The sky's light scale. */
  readonly light: number;
  readonly echo: number;
  /** The lamps' warm light on the kart, 0 to 1; a slow swell from lamp to lamp that never goes dark, so it never strobes. */
  readonly lampGlow: number;
  readonly ambience: Readonly<Record<AmbienceBed, number>>;
  readonly crowd: number;
}

/** Outside every zone: the course's own light and no ambience. */
export const OPEN_SKY: PresentationBlend = {
  light: 1,
  echo: 0,
  lampGlow: 0,
  ambience: { harbour: 0 },
  crowd: 0,
};

/** The lamps' glow between two lamps, as a share of the glow under one. */
const LAMP_GLOW_BETWEEN = 0.5;

/**
 * The blend at `splineProgress` metres from the start line on a lap of
 * `splineLength`. Overlapping zones multiply their light, so a tunnel's
 * dark eases off under the harbour's bright, and the louder of each other
 * part wins.
 */
export function presentationBlend(
  zones: readonly PresentationZone[],
  splineProgress: number,
  splineLength: number,
): PresentationBlend {
  let light = 1;
  let echo = 0;
  let lampGlow = 0;
  let harbour = 0;
  let crowd = 0;
  let touched = false;
  for (const zone of zones) {
    const weight = weightOf(zone, splineProgress, splineLength);
    if (weight <= 0) continue;
    touched = true;
    light *= 1 + weight * (zone.light - 1);
    echo = Math.max(echo, weight * zone.echo);
    if (zone.lamps !== null) {
      // Along the lap from the first lamp, so the swell runs on over the line.
      const along = wrap(splineProgress - zone.lamps.firstAt, splineLength);
      const phase = wrap(along, zone.lamps.spacing);
      const swell = 0.5 + 0.5 * Math.cos((2 * Math.PI * phase) / zone.lamps.spacing);
      lampGlow = Math.max(lampGlow, weight * (LAMP_GLOW_BETWEEN + (1 - LAMP_GLOW_BETWEEN) * swell));
    }
    harbour = Math.max(harbour, weight * zone.ambience.harbour);
    crowd = Math.max(crowd, weight * zone.crowd);
  }
  return touched ? { light, echo, lampGlow, ambience: { harbour }, crowd } : OPEN_SKY;
}

/** How far into `zone` the kart is at `s`: 0 outside, 1 wholly in, eased between. */
function weightOf(zone: PresentationZone, s: number, lap: number): number {
  // Every edge measured on from `from`, so a zone over the line reads as one run.
  const at = wrap(s - zone.from, lap);
  const fullFrom = wrap(zone.fullFrom - zone.from, lap);
  const fullTo = wrap(zone.fullTo - zone.from, lap);
  const to = wrap(zone.to - zone.from, lap);
  if (at >= to) return 0;
  if (at < fullFrom) return smoothstep(at / fullFrom);
  if (at <= fullTo) return 1;
  return smoothstep((to - at) / (to - fullTo));
}

function wrap(metres: number, length: number): number {
  const wrapped = metres % length;
  return wrapped < 0 ? wrapped + length : wrapped;
}

function smoothstep(x: number): number {
  return x * x * (3 - 2 * x);
}

// Monarch's edges, in metres from the line, are its layout's
// (assets/tracks/trackgen/layouts/monarch.py): a change there moves them here.
/** The tunnel's roof, 10 m past the mouth to 15 m short of the chicane. */
const MONARCH_ROOF_FROM_M = 678.6;
const MONARCH_ROOF_TO_M = 826.07;
/** `TUNNEL_LAMP_SPACING_M`, the first lamp half a spacing under the roof. */
const MONARCH_LAMP_SPACING_M = 12;
/** The start of `chicane-in`, of `harbour` and of `tabac`, and the end of `tabac`. */
const MONARCH_CHICANE_IN_M = 841.07;
const MONARCH_HARBOUR_FROM_M = 884.79;
const MONARCH_HARBOUR_TO_M = 953.79;
const MONARCH_TABAC_TO_M = 969.81;
/** How far either side of a portal the tunnel's dark eases in. */
const PORTAL_EASE_M = 10;

const NO_AMBIENCE = { harbour: 0 } as const;

// Bell Deer's edges, in metres from the line, are its layout's
// (assets/tracks/trackgen/layouts/bell_deer.py): a change there moves them here.
/** `BRIDGE_UNDERPASS_S`, where the underpass passes under the deck. */
const BELL_DEER_UNDERPASS_M = 573.8;
/** `BRIDGE_BACK_STRAIGHT_S`, where the back straight passes over the underpass. */
const BELL_DEER_BACK_STRAIGHT_M = 1020.7;
/**
 * Half the deck's width along the underpass, which it crosses square: the
 * run-off's half width and a wall (`runoff_half_width + WALL_THICKNESS_M`).
 */
const BELL_DEER_DECK_HALF_WIDTH_M = 12.5;
/** `BRIDGE_HILL_HALF_LENGTH_M`, the embankment's climb either side of the crest. */
const BELL_DEER_EMBANKMENT_HALF_M = 55;
/** How far short of the deck's edge the underside's dark eases in, and past it out. */
const DECK_EDGE_EASE_M = 12.5;
/** How far inside the deck's edge the underside is wholly dark. */
const DECK_EDGE_FULL_M = 2.5;
/** How far either side of the crest the deck's bright holds. */
const DECK_CREST_HOLD_M = 20;

const ZONES: Readonly<Record<TrackName, readonly PresentationZone[]>> = {
  "bell-deer": [
    {
      // Under the deck: dark and echoing from just short of its edge, all
      // light again before the runaway wheel's crossing.
      from: BELL_DEER_UNDERPASS_M - BELL_DEER_DECK_HALF_WIDTH_M - DECK_EDGE_EASE_M,
      fullFrom: BELL_DEER_UNDERPASS_M - BELL_DEER_DECK_HALF_WIDTH_M + DECK_EDGE_FULL_M,
      fullTo: BELL_DEER_UNDERPASS_M + BELL_DEER_DECK_HALF_WIDTH_M - DECK_EDGE_FULL_M,
      to: BELL_DEER_UNDERPASS_M + BELL_DEER_DECK_HALF_WIDTH_M + DECK_EDGE_EASE_M,
      light: 0.55,
      echo: 1,
      lamps: null,
      ambience: NO_AMBIENCE,
      crowd: 0,
    },
    {
      // Over it a minute later: brighter than the course's own sky up the
      // embankment, brightest across the crest, where the woods and the
      // Circuit Wheel open out ahead.
      from: BELL_DEER_BACK_STRAIGHT_M - BELL_DEER_EMBANKMENT_HALF_M,
      fullFrom: BELL_DEER_BACK_STRAIGHT_M - DECK_CREST_HOLD_M,
      fullTo: BELL_DEER_BACK_STRAIGHT_M + DECK_CREST_HOLD_M,
      to: BELL_DEER_BACK_STRAIGHT_M + BELL_DEER_EMBANKMENT_HALF_M,
      light: 1.12,
      echo: 0,
      lamps: null,
      ambience: NO_AMBIENCE,
      crowd: 0,
    },
  ],
  goldstone: [],
  monarch: [
    {
      // Dark, echoing and lit by its lamps; the dark comes in across the
      // mouth's portal and lifts across the exit's.
      from: MONARCH_ROOF_FROM_M - PORTAL_EASE_M,
      fullFrom: MONARCH_ROOF_FROM_M + PORTAL_EASE_M,
      fullTo: MONARCH_ROOF_TO_M - PORTAL_EASE_M,
      to: MONARCH_CHICANE_IN_M,
      light: 0.45,
      echo: 1,
      lamps: {
        firstAt: MONARCH_ROOF_FROM_M + MONARCH_LAMP_SPACING_M / 2,
        spacing: MONARCH_LAMP_SPACING_M,
      },
      ambience: NO_AMBIENCE,
      crowd: 0,
    },
    {
      // The release: brighter than the course's own sky from the exit
      // portal through the chicane to the harbour, the water and the
      // harbour's stand with it, gone by Tabac's exit.
      from: MONARCH_ROOF_TO_M,
      fullFrom: MONARCH_HARBOUR_FROM_M,
      fullTo: MONARCH_HARBOUR_TO_M,
      to: MONARCH_TABAC_TO_M,
      light: 1.15,
      echo: 0,
      lamps: null,
      ambience: { harbour: 1 },
      crowd: 0.6,
    },
  ],
  oval: [],
};

/** The zones of `track`'s course, in the order they come along the lap. */
export function presentationZonesOf(track: TrackName): readonly PresentationZone[] {
  return ZONES[track];
}
