// The tracks the client can race on. Each is a glTF the View draws and a
// sidecar the sim reads, exported together from Blender (ADR 0007).
import bellDeerGltf from "../../../assets/tracks/bell-deer/bell-deer.glb?url";
import bellDeerSidecar from "../../../assets/tracks/bell-deer/bell-deer.track.json?url";
import goldstoneGltf from "../../../assets/tracks/goldstone/goldstone.glb?url";
import goldstoneSidecar from "../../../assets/tracks/goldstone/goldstone.track.json?url";
import monarchGltf from "../../../assets/tracks/monarch/monarch.glb?url";
import monarchSidecar from "../../../assets/tracks/monarch/monarch.track.json?url";
import ovalGltf from "../../../assets/tracks/oval/oval.glb?url";
import ovalSidecar from "../../../assets/tracks/oval/oval.track.json?url";

import type { Picks } from "./generated/sim";

export type TrackName = "bell-deer" | "goldstone" | "monarch" | "oval";

/** A point in the world's metres: x, y up, z. */
export type WorldPoint = readonly [number, number, number];

export interface TrackFiles {
  readonly name: TrackName;
  /** The mesh, for the View. */
  readonly gltfUrl: string;
  /** The data, for the sim; fetch it and hand the text to the bridge. */
  readonly sidecarUrl: string;
}

export const TRACKS: Readonly<Record<TrackName, TrackFiles>> = {
  "bell-deer": {
    name: "bell-deer",
    gltfUrl: bellDeerGltf,
    sidecarUrl: bellDeerSidecar,
  },
  goldstone: { name: "goldstone", gltfUrl: goldstoneGltf, sidecarUrl: goldstoneSidecar },
  monarch: { name: "monarch", gltfUrl: monarchGltf, sidecarUrl: monarchSidecar },
  oval: { name: "oval", gltfUrl: ovalGltf, sidecarUrl: ovalSidecar },
};

/** The track raced by default; the oval is a test fixture. */
export const DEFAULT_TRACK: TrackName = "bell-deer";

/** A flag the Title Scene's picker and the Room Scene's badge draw. */
export type FlagName = "jp" | "gb" | "mc";

/** A track as players see it: its name and flag, and the real circuit it is after. */
export interface Course {
  readonly track: TrackName;
  readonly name: string;
  /** The real circuit the course is drawn after. */
  readonly after: string;
  readonly flag: FlagName;
  /** One line on what the lap is like. */
  readonly blurb: string;
}

/**
 * Every course a creator can pick, in the picker's order; a track that is
 * not here, the oval, can still be raced from `?track=`.
 */
export const COURSES: readonly Course[] = [
  {
    track: "bell-deer",
    name: "Bell Deer",
    after: "Suzuka",
    flag: "jp",
    blurb: "A figure eight over the bridge, the esses, Spoon and 130R",
  },
  {
    track: "goldstone",
    name: "Goldstone",
    after: "Silverstone",
    flag: "gb",
    blurb: "Flat out and open, Maggotts and Becketts, the Hangar Straight",
  },
  {
    track: "monarch",
    name: "Monarch",
    after: "Monaco",
    flag: "mc",
    blurb: "Walls close, up to Casino Square, down through the tunnel",
  },
];

/** The course racing `track`, or `undefined` for a track no course is, the oval or one this client lacks. */
export function courseOf(track: string): Course | undefined {
  return COURSES.find((course) => course.track === track);
}

const COURSE_STORAGE_KEY = "course";

/** What was saved under `key` last time, or `null` when nothing was or the storage refuses, which reads as nothing saved. */
function remembered(storage: Storage, key: string): string | null {
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}

/** Remember `value` under `key` for the next visit; storage that refuses, a private window or a full store, is let be. */
function remember(storage: Storage, key: string, value: string): void {
  try {
    storage.setItem(key, value);
  } catch {
    // The next visit loads what was saved before.
  }
}

/** The course picked last time, kept beside the audio toggles; the default's when unsaved, unreadable or no course. */
export function loadCoursePick(storage: Storage): Course {
  // The default track is always a course, so the cast cannot hide an undefined.
  return (
    courseOf(remembered(storage, COURSE_STORAGE_KEY) ?? "") ?? (courseOf(DEFAULT_TRACK) as Course)
  );
}

export function saveCoursePick(storage: Storage, course: Course): void {
  remember(storage, COURSE_STORAGE_KEY, course.track);
}

/**
 * What a creator is offered of each pick: the key it is remembered and asked
 * for under, and the values it can take. Every pick is read and written
 * through this, so one added to the sim's `Picks` needs nothing here.
 */
export type PickOffer = {
  readonly [Pick in keyof Picks]: { readonly key: string; readonly values: readonly Picks[Pick][] };
};

/** Every pick's name, in `offer`'s order. */
function pickNames(offer: PickOffer): (keyof Picks)[] {
  return Object.keys(offer) as (keyof Picks)[];
}

/** The value of `pick` spelled `text`, when a creator is offered it; storage and the query string hold text. */
function offered<Pick extends keyof Picks>(
  offer: PickOffer,
  pick: Pick,
  text: string | null,
): Picks[Pick] | undefined {
  const values: readonly Picks[Pick][] = offer[pick].values;
  return values.find((value) => String(value) === text);
}

/**
 * The picks made last time, kept beside the course pick, each under its own
 * key; `fresh`'s for any unsaved, unreadable or no longer offered.
 */
export function loadPicks(storage: Storage, offer: PickOffer, fresh: Picks): Picks {
  const picks: Partial<Record<keyof Picks, unknown>> = {};
  for (const pick of pickNames(offer)) {
    picks[pick] = offered(offer, pick, remembered(storage, offer[pick].key)) ?? fresh[pick];
  }
  // Every pick of `offer` was set above, and the offer names every pick.
  return picks as Picks;
}

export function savePicks(storage: Storage, offer: PickOffer, picks: Picks): void {
  for (const pick of pickNames(offer)) remember(storage, offer[pick].key, String(picks[pick]));
}

/**
 * The picks the page's query string asks for under their keys (`?bots=ace`,
 * `?tyres=on`, `?laps=5`), so a smoke run can load any into a solo race
 * beside `?track=`; one asked for as nothing a creator is offered is left out.
 */
export function picksFromSearch(search: string, offer: PickOffer): Partial<Picks> {
  const params = new URLSearchParams(search);
  const picks: Partial<Record<keyof Picks, unknown>> = {};
  for (const pick of pickNames(offer)) {
    const value = offered(offer, pick, params.get(offer[pick].key));
    if (value !== undefined) picks[pick] = value;
  }
  // Each value set above is one `offer` holds for its pick.
  return picks as Partial<Picks>;
}

/**
 * Pick a track from the page's query string (`?track=oval`,
 * `?track=monarch`) so a tester can load any without a build. Anything else is the default.
 */
export function selectTrack(search: string): TrackFiles {
  return trackFromSearch(search) ?? TRACKS[DEFAULT_TRACK];
}

/** The track the page's query string names, or `undefined` when it names none the client has. */
export function trackFromSearch(search: string): TrackFiles | undefined {
  const requested = new URLSearchParams(search).get("track");
  return requested !== null && isTrackName(requested) ? TRACKS[requested] : undefined;
}

export function isTrackName(name: string): name is TrackName {
  return Object.hasOwn(TRACKS, name);
}

export async function fetchSidecar(track: TrackFiles): Promise<string> {
  const response = await fetch(track.sidecarUrl);
  if (!response.ok) {
    throw new Error(`could not load the ${track.name} sidecar: ${response.status}`);
  }
  return response.text();
}

/**
 * The centreline the course map draws, read from the sidecar's text. The
 * rest of the sidecar is the sim's to read; the client takes only this.
 * Throws when the text has no centreline of 3-vectors.
 */
export function centrelineOf(sidecar: string): readonly WorldPoint[] {
  const parsed: unknown = JSON.parse(sidecar);
  const centreline =
    typeof parsed === "object" && parsed !== null && "centreline" in parsed
      ? parsed.centreline
      : undefined;
  if (
    !Array.isArray(centreline) ||
    !centreline.every(
      (point) => Array.isArray(point) && point.length === 3 && point.every(Number.isFinite),
    )
  ) {
    throw new TypeError("the sidecar has no centreline of 3-vectors");
  }
  return centreline as WorldPoint[];
}

/** A lap's length along `centreline`, climbs included, closing the loop back to the line, as the sim measures spline progress. */
export function courseLength(centreline: readonly WorldPoint[]): number {
  let metres = 0;
  centreline.forEach(([x, y, z], i) => {
    const [nextX, nextY, nextZ] = centreline[(i + 1) % centreline.length] ?? [x, y, z];
    metres += Math.hypot(nextX - x, nextY - y, nextZ - z);
  });
  return metres;
}

/** A pit box (ADR 0081): where its slot's kart stands for a stop, and a yaw about +Y, zero facing -Z, down the lane. */
export interface PitBox {
  readonly position: WorldPoint;
  readonly yaw: number;
  /** Which side of the kart on its mark the pit building stands, 1 for its right and -1 for its left, where the crew wait and the light stands, off the fast lane. */
  readonly buildingSide: 1 | -1;
}

/** The course's pit lane as the Race Scene draws it: its path for the course map and its pit boxes for the View. */
export interface PitLaneLayout {
  readonly path: readonly WorldPoint[];
  /** One per grid slot, in slot order. */
  readonly pitBoxes: readonly PitBox[];
}

/**
 * The course's pit lane, read from the sidecar's text; `undefined` on a
 * course without one (ADR 0081).
 */
export function pitLaneOf(sidecar: string): PitLaneLayout | undefined {
  const parsed = JSON.parse(sidecar) as {
    pit_lane?: {
      path: readonly WorldPoint[];
      pit_boxes?: readonly Pick<PitBox, "position" | "yaw">[];
    };
  };
  const lane = parsed.pit_lane;
  if (lane === undefined) return undefined;
  return {
    path: lane.path,
    pitBoxes: (lane.pit_boxes ?? []).map((box) => ({
      ...box,
      buildingSide: sideOfThePath(lane.path, box.position),
    })),
  };
}

/** Which side of the path, as it runs, `point` stands on: 1 for its right, -1 for its left. */
function sideOfThePath(path: readonly WorldPoint[], point: readonly number[]): 1 | -1 {
  const [px = 0, , pz = 0] = point;
  let nearest = Number.POSITIVE_INFINITY;
  let side: 1 | -1 = 1;
  for (let i = 0; i + 1 < path.length; i += 1) {
    const [ax = 0, , az = 0] = path[i] ?? [];
    const [bx = 0, , bz = 0] = path[i + 1] ?? [];
    const distance = (ax + bx - 2 * px) ** 2 + (az + bz - 2 * pz) ** 2;
    if (distance < nearest) {
      nearest = distance;
      // Right of the direction of travel (x, z), seen from above, is (-z, x).
      side = -(bz - az) * (px - ax) + (bx - ax) * (pz - az) >= 0 ? 1 : -1;
    }
  }
  return side;
}
