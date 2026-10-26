import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { type GltfJson, gltfJson } from "./gltf-json";

/** The exported track as the View loads it. */
function track(name: string): GltfJson {
  return gltfJson(
    readFileSync(new URL(`../../../assets/tracks/${name}/${name}.glb`, import.meta.url)),
  );
}

function nodeNames(gltf: GltfJson): string[] {
  return gltf.nodes.map(({ name }) => name ?? "");
}

function attributesOf(gltf: GltfJson, nodeName: string): string[] {
  const node = gltf.nodes.find(({ name }) => name === nodeName);
  const mesh = node?.mesh === undefined ? undefined : gltf.meshes[node.mesh];
  return Object.keys(mesh?.primitives[0]?.attributes ?? {});
}

describe.each(["brands-patch", "oval"])("%s's boost pads", (name) => {
  const gltf = track(name);
  const sidecar = JSON.parse(
    readFileSync(
      new URL(`../../../assets/tracks/${name}/${name}.track.json`, import.meta.url),
      "utf8",
    ),
  ) as { boost_pads: { position: [number, number, number]; yaw: number }[] };

  it("draws a carbon plate over each pad, centred on it", () => {
    const plates = gltf.nodes.filter(({ name }) => name?.endsWith(".pad-carbon"));
    expect(plates).toHaveLength(sidecar.boost_pads.length);
    sidecar.boost_pads.forEach(({ position: [x, , z] }, number) => {
      const plate = plates.find(
        ({ name }) => name === `BoostPad.${String(number).padStart(2, "0")}.pad-carbon`,
      );
      const mesh = plate?.mesh === undefined ? undefined : gltf.meshes[plate.mesh];
      const bounds = gltf.accessors[mesh?.primitives[0]?.attributes.POSITION ?? -1];
      const [minX, , minZ] = bounds?.min ?? [];
      const [maxX, , maxZ] = bounds?.max ?? [];
      expect(((minX ?? Number.NaN) + (maxX ?? Number.NaN)) / 2).toBeCloseTo(x, 2);
      expect(((minZ ?? Number.NaN) + (maxZ ?? Number.NaN)) / 2).toBeCloseTo(z, 2);
    });
  });
});

describe("Brands Patch's final art", () => {
  const brandsPatch = track("brands-patch");

  it("dresses the track: kerbs on the corners, the start line, hoardings, stands and the gantry", () => {
    const names = nodeNames(brandsPatch);
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
    expect(attributesOf(brandsPatch, "Asphalt")).toContain("TEXCOORD_0");
  });

  it("is flat-coloured, with no texture to fetch", () => {
    expect(brandsPatch.images ?? []).toHaveLength(0);
  });
});

describe("the oval fixture", () => {
  it("stays plain: kerbs and a start line, no hoardings or stands to draw in the sim's tests", () => {
    const families = new Set(nodeNames(track("oval")).map((name) => name.split(".")[0]));
    expect(families).toContain("Kerb");
    expect(families).toContain("StartLine");
    expect(families).not.toContain("Hoarding");
    expect(families).not.toContain("Grandstand");
  });
});
