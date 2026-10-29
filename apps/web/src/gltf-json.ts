// The JSON chunk of a binary glTF, for the tests that hold the exported
// assets to their contracts without a renderer: a glb is a 12-byte header,
// a JSON chunk and a binary chunk, and the JSON carries the node names,
// materials and each mesh's attributes and bounds.

export interface GltfJson {
  readonly nodes: readonly {
    readonly name?: string;
    readonly mesh?: number;
    readonly children?: readonly number[];
    readonly translation?: readonly [number, number, number];
    readonly rotation?: readonly [number, number, number, number];
  }[];
  readonly meshes: readonly {
    readonly primitives: readonly {
      readonly attributes: { readonly POSITION: number } & Readonly<Record<string, number>>;
      readonly material?: number;
    }[];
  }[];
  readonly materials?: readonly { readonly name?: string }[];
  readonly accessors: readonly { readonly min?: number[]; readonly max?: number[] }[];
  readonly images?: readonly unknown[];
  readonly textures?: readonly unknown[];
}

/** Parse a glb's JSON chunk. Throws on anything but a glb whose first chunk is JSON. */
export function gltfJson(bytes: Buffer): GltfJson {
  if (bytes.toString("ascii", 0, 4) !== "glTF" || bytes.toString("ascii", 16, 20) !== "JSON") {
    throw new Error("not a binary glTF with a leading JSON chunk");
  }
  const jsonLength = bytes.readUInt32LE(12);
  return JSON.parse(bytes.toString("utf8", 20, 20 + jsonLength)) as GltfJson;
}
