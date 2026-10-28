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
    const plates = gltf.nodes.filter(
      ({ name }) => name?.startsWith("BoostPad.") && name.endsWith(".carbon"),
    );
    expect(plates).toHaveLength(sidecar.boost_pads.length);
    sidecar.boost_pads.forEach(({ position: [x, , z] }, number) => {
      const plate = plates.find(
        ({ name }) => name === `BoostPad.${String(number).padStart(2, "0")}.carbon`,
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

describe.each(["brands-patch", "oval"])("%s's sausage kerbs", (name) => {
  const gltf = track(name);
  const sidecar = JSON.parse(
    readFileSync(
      new URL(`../../../assets/tracks/${name}/${name}.track.json`, import.meta.url),
      "utf8",
    ),
  ) as { kerbs: { position: [number, number, number]; yaw: number }[] };

  it("draws a yellow capsule over each kerb, centred on it", () => {
    const capsules = gltf.nodes.filter(
      ({ name }) => name?.startsWith("SausageKerb.") && name.endsWith(".kerb-yellow"),
    );
    expect(capsules).toHaveLength(sidecar.kerbs.length);
    sidecar.kerbs.forEach(({ position: [x, , z] }, number) => {
      const capsule = capsules.find(
        ({ name }) => name === `SausageKerb.${String(number).padStart(2, "0")}.kerb-yellow`,
      );
      const mesh = capsule?.mesh === undefined ? undefined : gltf.meshes[capsule.mesh];
      const bounds = gltf.accessors[mesh?.primitives[0]?.attributes.POSITION ?? -1];
      const [minX, , minZ] = bounds?.min ?? [];
      const [maxX, , maxZ] = bounds?.max ?? [];
      expect(((minX ?? Number.NaN) + (maxX ?? Number.NaN)) / 2).toBeCloseTo(x, 2);
      expect(((minZ ?? Number.NaN) + (maxZ ?? Number.NaN)) / 2).toBeCloseTo(z, 2);
    });
  });
});

describe.each(["brands-patch", "oval"])("%s's marshal posts", (name) => {
  const gltf = track(name);
  const sidecar = JSON.parse(
    readFileSync(
      new URL(`../../../assets/tracks/${name}/${name}.track.json`, import.meta.url),
      "utf8",
    ),
  ) as { wheel_crossings: { position: [number, number, number]; half_width: number }[] };

  it("stands a post and its lamp beside each wheel crossing", () => {
    const posts = gltf.nodes.filter(({ name }) => name?.startsWith("MarshalPost."));
    expect(posts).toHaveLength(2 * sidecar.wheel_crossings.length);
    sidecar.wheel_crossings.forEach(({ position: [x, , z], half_width }, number) => {
      const prefix = `MarshalPost.${String(number).padStart(2, "0")}.`;
      for (const part of ["board", "chevron"]) {
        const node = posts.find(({ name }) => name === `${prefix}${part}`);
        const mesh = node?.mesh === undefined ? undefined : gltf.meshes[node.mesh];
        const bounds = gltf.accessors[mesh?.primitives[0]?.attributes.POSITION ?? -1];
        const [minX, , minZ] = bounds?.min ?? [];
        const [maxX, , maxZ] = bounds?.max ?? [];
        const centre = [
          ((minX ?? Number.NaN) + (maxX ?? Number.NaN)) / 2,
          ((minZ ?? Number.NaN) + (maxZ ?? Number.NaN)) / 2,
        ] as const;
        // Beside the crossing: off the asphalt's middle, inside the walls.
        const away = Math.hypot(centre[0] - x, centre[1] - z);
        expect(away).toBeGreaterThan(2);
        expect(away).toBeLessThan(half_width);
      }
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

  it("builds the stands and the gantry in the track's own colours, each part named for its material", () => {
    const parts = brandsPatch.nodes.filter(
      ({ name }) => name?.startsWith("Grandstand.") || name?.startsWith("Gantry."),
    );
    expect(parts.length).toBeGreaterThan(0);
    for (const { name, mesh } of parts) {
      const primitives = mesh === undefined ? [] : (brandsPatch.meshes[mesh]?.primitives ?? []);
      const materials = primitives.map(
        ({ material }) => brandsPatch.materials?.[material ?? -1]?.name,
      );
      expect(materials, name).toEqual([name?.split(".")[1]]);
    }
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
