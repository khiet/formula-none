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

import type { BotLevel, RaceDistance, Tyres } from "./generated/sim";

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

/** A bot level as the Title Scene offers it: its caption and the line describing the pick. */
export interface BotLevelChoice {
  readonly level: BotLevel;
  readonly name: string;
  /** How the bots drive at this level, in the course blurb's voice, after "Name: ". */
  readonly blurb: string;
}

/** The three levels in the Title Scene's order, Basic first (ADR 0078). */
export const BOT_LEVELS: readonly BotLevelChoice[] = [
  { level: "basic", name: "Basic", blurb: "lift for corners, hold an item, wait for you" },
  { level: "pro", name: "Pro", blurb: "carry speed, fire sooner, wait less" },
  { level: "ace", name: "Ace", blurb: "corner flat out, fire on sight, never wait for you" },
];

const DEFAULT_BOT_LEVEL: BotLevel = "basic";
const BOT_LEVEL_STORAGE_KEY = "bots";

export function isBotLevel(name: string): name is BotLevel {
  return BOT_LEVELS.some((choice) => choice.level === name);
}

/** The bot level picked last time, kept beside the course pick; Basic when unsaved, unreadable or no level. */
export function loadBotLevel(storage: Storage): BotLevel {
  const saved = remembered(storage, BOT_LEVEL_STORAGE_KEY);
  return saved !== null && isBotLevel(saved) ? saved : DEFAULT_BOT_LEVEL;
}

export function saveBotLevel(storage: Storage, level: BotLevel): void {
  remember(storage, BOT_LEVEL_STORAGE_KEY, level);
}

/**
 * The bot level the page's query string asks for (`?bots=ace`), so a smoke
 * run can load any level into a solo race beside `?track=`; `undefined` for
 * none or an unknown one.
 */
export function botLevelFromSearch(search: string): BotLevel | undefined {
  const requested = new URLSearchParams(search).get("bots");
  return requested !== null && isBotLevel(requested) ? requested : undefined;
}

/** A race distance as the Title Scene offers it: its caption and the line describing it. */
export interface DistanceChoice {
  readonly distance: RaceDistance;
  readonly name: string;
  /** What the pick means for the race, in the course blurb's voice, after "Name: ". */
  readonly blurb: string;
}

/** The picks in the Title Scene's order, the Sprint first (ADR 0083). */
export const DISTANCE_CHOICES: readonly DistanceChoice[] = [
  { distance: "sprint", name: "Sprint", blurb: "the short race, flat out from the lights" },
  { distance: "grand-prix", name: "Grand Prix", blurb: "the long race, more laps to manage" },
];

const DEFAULT_DISTANCE: RaceDistance = "sprint";
const DISTANCE_STORAGE_KEY = "distance";

export function isRaceDistance(name: string): name is RaceDistance {
  return DISTANCE_CHOICES.some((choice) => choice.distance === name);
}

/** The race distance picked last time, kept beside the course pick; a Sprint when unsaved, unreadable or no pick. */
export function loadDistance(storage: Storage): RaceDistance {
  const saved = remembered(storage, DISTANCE_STORAGE_KEY);
  return saved !== null && isRaceDistance(saved) ? saved : DEFAULT_DISTANCE;
}

export function saveDistance(storage: Storage, distance: RaceDistance): void {
  remember(storage, DISTANCE_STORAGE_KEY, distance);
}

/**
 * The race distance the page's query string asks for (`?distance=grand-prix`),
 * so a smoke run can load either into a solo race beside `?track=`; `undefined`
 * for none or an unknown one.
 */
export function distanceFromSearch(search: string): RaceDistance | undefined {
  const requested = new URLSearchParams(search).get("distance");
  return requested !== null && isRaceDistance(requested) ? requested : undefined;
}

/** A Tyres pick as the Title Scene offers it: its caption and the line describing it. */
export interface TyresChoice {
  readonly tyres: Tyres;
  readonly name: string;
  /** What the pick means for the race, in the course blurb's voice, after "Name: ". */
  readonly blurb: string;
}

/** The picks in the Title Scene's order, Off first (ADR 0083). */
export const TYRES_CHOICES: readonly TyresChoice[] = [
  { tyres: "off", name: "Off", blurb: "they last the race" },
  { tyres: "on", name: "On", blurb: "driving wears them" },
];

const DEFAULT_TYRES: Tyres = "off";
const TYRES_STORAGE_KEY = "tyres";

export function isTyres(name: string): name is Tyres {
  return TYRES_CHOICES.some((choice) => choice.tyres === name);
}

/** The Tyres pick made last time, kept beside the course pick; Off when unsaved, unreadable or no pick. */
export function loadTyres(storage: Storage): Tyres {
  const saved = remembered(storage, TYRES_STORAGE_KEY);
  return saved !== null && isTyres(saved) ? saved : DEFAULT_TYRES;
}

export function saveTyres(storage: Storage, tyres: Tyres): void {
  remember(storage, TYRES_STORAGE_KEY, tyres);
}

/**
 * The Tyres pick the page's query string asks for (`?tyres=on`), so a smoke
 * run can load either into a solo race beside `?track=`; `undefined` for
 * none or an unknown one.
 */
export function tyresFromSearch(search: string): Tyres | undefined {
  const requested = new URLSearchParams(search).get("tyres");
  return requested !== null && isTyres(requested) ? requested : undefined;
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

/**
 * The path of the course's pit lane the course map draws, read from the
 * sidecar's text; `undefined` on a course without one (ADR 0081).
 */
export function pitLaneOf(sidecar: string): readonly WorldPoint[] | undefined {
  const parsed = JSON.parse(sidecar) as { pit_lane?: { path: readonly WorldPoint[] } };
  return parsed.pit_lane?.path;
}
