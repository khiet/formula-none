// The tracks the client can race on. Each is a glTF the View draws and a
// sidecar the sim reads, exported together from Blender (ADR 0007).
import bellDeerGltf from "../../../assets/tracks/bell-deer/bell-deer.glb?url";
import bellDeerSidecar from "../../../assets/tracks/bell-deer/bell-deer.track.json?url";
import ovalGltf from "../../../assets/tracks/oval/oval.glb?url";
import ovalSidecar from "../../../assets/tracks/oval/oval.track.json?url";

export type TrackName = "bell-deer" | "oval";

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
  oval: { name: "oval", gltfUrl: ovalGltf, sidecarUrl: ovalSidecar },
};

/** The track raced by default; the oval is a test fixture. */
export const DEFAULT_TRACK: TrackName = "bell-deer";

/**
 * Pick a track from the page's query string (`?track=oval`) so a tester can
 * load either without a build. Anything else is the default.
 */
export function selectTrack(search: string): TrackFiles {
  const requested = new URLSearchParams(search).get("track");
  const name = requested !== null && isTrackName(requested) ? requested : DEFAULT_TRACK;
  return TRACKS[name];
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
