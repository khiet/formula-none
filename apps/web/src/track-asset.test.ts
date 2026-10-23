import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The exported tracks as the View loads them, parsed by hand from the glb's
// JSON chunk: the node names and the attributes each mesh carries are what
// these tests ask about.
interface Gltf {
  readonly nodes: readonly { readonly name?: string; readonly mesh?: number }[];
  readonly meshes: readonly {
    readonly primitives: readonly { readonly attributes: Record<string, number> }[];
  }[];
  readonly images?: readonly unknown[];
}

function gltfJson(track: string): Gltf {
  const bytes = readFileSync(
    new URL(`../../../assets/tracks/${track}/${track}.glb`, import.meta.url),
  );
  expect(bytes.toString("ascii", 0, 4)).toBe("glTF");
  const jsonLength = bytes.readUInt32LE(12);
  return JSON.parse(bytes.toString("utf8", 20, 20 + jsonLength)) as Gltf;
}

function nodeNames(gltf: Gltf): string[] {
  return gltf.nodes.map(({ name }) => name ?? "");
}

function attributesOf(gltf: Gltf, nodeName: string): string[] {
  const node = gltf.nodes.find(({ name }) => name === nodeName);
  const mesh = node?.mesh === undefined ? undefined : gltf.meshes[node.mesh];
  return Object.keys(mesh?.primitives[0]?.attributes ?? {});
}

describe("Brands Patch's final art", () => {
  const track = gltfJson("brands-patch");

  it("dresses the track: kerbs on the corners, the start line, hoardings, stands and the gantry", () => {
    const names = nodeNames(track);
    const families = new Set(names.map((name) => name.split(".")[0]));
    for (const family of ["Kerb", "StartLine", "Hoarding", "Sponsor", "Grandstand", "Gantry"]) {
      expect(families, `no ${family} in the glTF`).toContain(family);
    }
    // Every hoarding carries a sponsor.
    expect(names.filter((name) => name.startsWith("Sponsor."))).toHaveLength(
      names.filter((name) => name.startsWith("Hoarding.")).length,
    );
  });

  it("maps the asphalt for tyre marks: a texture coordinate on every vertex", () => {
    expect(attributesOf(track, "Asphalt")).toContain("TEXCOORD_0");
  });

  it("is flat-coloured, with no texture to fetch", () => {
    expect(track.images ?? []).toHaveLength(0);
  });
});

describe("the oval fixture", () => {
  it("stays plain: kerbs and a start line, no hoardings or stands to draw in the sim's tests", () => {
    const families = new Set(nodeNames(gltfJson("oval")).map((name) => name.split(".")[0]));
    expect(families).toContain("Kerb");
    expect(families).toContain("StartLine");
    expect(families).not.toContain("Hoarding");
    expect(families).not.toContain("Grandstand");
  });
});
