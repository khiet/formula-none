// The tracks the client can race on. Each is a glTF the View draws and a
// sidecar the sim reads, exported together from Blender (ADR 0007).
import brandsPatchGltf from "../../../assets/tracks/brands-patch/brands-patch.glb?url";
import brandsPatchSidecar from "../../../assets/tracks/brands-patch/brands-patch.track.json?url";
import ovalGltf from "../../../assets/tracks/oval/oval.glb?url";
import ovalSidecar from "../../../assets/tracks/oval/oval.track.json?url";

export type TrackName = "brands-patch" | "oval";

export interface TrackFiles {
  readonly name: TrackName;
  /** The mesh, for the View. */
  readonly gltfUrl: string;
  /** The data, for the sim; fetch it and hand the text to the bridge. */
  readonly sidecarUrl: string;
}

export const TRACKS: Readonly<Record<TrackName, TrackFiles>> = {
  "brands-patch": {
    name: "brands-patch",
    gltfUrl: brandsPatchGltf,
    sidecarUrl: brandsPatchSidecar,
  },
  oval: { name: "oval", gltfUrl: ovalGltf, sidecarUrl: ovalSidecar },
};

/** The track raced by default; the oval is a test fixture. */
export const DEFAULT_TRACK: TrackName = "brands-patch";

/**
 * Pick a track from the page's query string (`?track=oval`) so a tester can
 * load either without a build. Anything else is the default.
 */
export function selectTrack(search: string): TrackFiles {
  const requested = new URLSearchParams(search).get("track");
  const name = requested !== null && isTrackName(requested) ? requested : DEFAULT_TRACK;
  return TRACKS[name];
}

function isTrackName(name: string): name is TrackName {
  return Object.hasOwn(TRACKS, name);
}

export async function fetchSidecar(track: TrackFiles): Promise<string> {
  const response = await fetch(track.sidecarUrl);
  if (!response.ok) {
    throw new Error(`could not load the ${track.name} sidecar: ${response.status}`);
  }
  return response.text();
}
