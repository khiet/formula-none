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

/** The course picked last time, kept beside the audio toggles; the default's when unsaved, unreadable or no course. */
export function loadCoursePick(storage: Storage): Course {
  let saved: string | null = null;
  try {
    saved = storage.getItem(COURSE_STORAGE_KEY);
  } catch {
    // Storage that refuses reads as nothing saved.
  }
  // The default track is always a course, so the cast cannot hide an undefined.
  return courseOf(saved ?? "") ?? (courseOf(DEFAULT_TRACK) as Course);
}

/** Remember the pick for the next visit; storage that refuses is let be. */
export function saveCoursePick(storage: Storage, course: Course): void {
  try {
    storage.setItem(COURSE_STORAGE_KEY, course.track);
  } catch {
    // A private window or a full store; the next visit loads what was saved before.
  }
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
