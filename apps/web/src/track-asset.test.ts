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

/** The names of the materials a node's mesh paints with, sorted. */
function materialsOf(gltf: GltfJson, nodeName: string): (string | undefined)[] {
  const node = gltf.nodes.find(({ name }) => name === nodeName);
  const primitives = node?.mesh === undefined ? [] : (gltf.meshes[node.mesh]?.primitives ?? []);
  return primitives.map(({ material }) => gltf.materials?.[material ?? -1]?.name).sort();
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

  it("stands a post, its band and its lamp beside each wheel crossing", () => {
    const posts = gltf.nodes.filter(({ name }) => name?.startsWith("MarshalPost."));
    expect(posts).toHaveLength(3 * sidecar.wheel_crossings.length);
    sidecar.wheel_crossings.forEach(({ position: [x, , z], half_width }, number) => {
      const prefix = `MarshalPost.${String(number).padStart(2, "0")}.`;
      for (const part of ["board", "post-band", "chevron"]) {
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

describe.each(["brands-patch", "oval"])("%s's hazard zones", (name) => {
  const gltf = track(name);
  const sidecar = JSON.parse(
    readFileSync(
      new URL(`../../../assets/tracks/${name}/${name}.track.json`, import.meta.url),
      "utf8",
    ),
  ) as { wheel_crossings: { position: [number, number, number] }[] };

  it("paints a zone of tint and chevrons over each wheel crossing, which the View flashes", () => {
    const zones = gltf.nodes.filter(({ name }) => name?.startsWith("HazardZone."));
    const materials = ["hazard-asphalt", "hazard-sand", "kerb-yellow", "kerb-black"];
    expect(zones).toHaveLength(materials.length * sidecar.wheel_crossings.length);
    sidecar.wheel_crossings.forEach(({ position: [x, , z] }, number) => {
      const prefix = `HazardZone.${String(number).padStart(2, "0")}.`;
      const tint = zones.find(({ name }) => name === `${prefix}hazard-asphalt`);
      const mesh = tint?.mesh === undefined ? undefined : gltf.meshes[tint.mesh];
      const bounds = gltf.accessors[mesh?.primitives[0]?.attributes.POSITION ?? -1];
      const [minX = Number.NaN, , minZ = Number.NaN] = bounds?.min ?? [];
      const [maxX = Number.NaN, , maxZ = Number.NaN] = bounds?.max ?? [];
      // Over the crossing.
      expect(x).toBeGreaterThan(minX);
      expect(x).toBeLessThan(maxX);
      expect(z).toBeGreaterThan(minZ);
      expect(z).toBeLessThan(maxZ);
      for (const material of materials) {
        expect(zones.map(({ name }) => name)).toContain(`${prefix}${material}`);
      }
    });
  });
});

describe("Suzuqa Patch's final art", () => {
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

  it("stands the Circuit Wheel on a rim node with its sixteen gondolas hung under it", () => {
    const rim = brandsPatch.nodes.find(({ name }) => name === "Prop.circuit-wheel.1.rim");
    expect(rim).toBeDefined();
    // Its origin at the hub, 14 m over the grass, and a mesh of its own to turn.
    expect(rim?.translation?.[1]).toBeCloseTo(14 - 0.05, 2);
    expect(rim?.mesh).toBeDefined();
    const children = (rim?.children ?? []).flatMap((index) => brandsPatch.nodes[index] ?? []);
    const gondolas = children.filter(({ name }) =>
      name?.startsWith("Prop.circuit-wheel.1.gondola."),
    );
    expect(gondolas.map(({ name }) => name).sort()).toEqual(
      Array.from({ length: 16 }, (_, k) => `Prop.circuit-wheel.1.gondola.${k + 1}`).sort(),
    );
    // Each gondola's origin is its hanger, 12 m out from the hub in the rim's own frame.
    for (const gondola of gondolas) {
      const [x = Number.NaN, y = Number.NaN, z = Number.NaN] = gondola.translation ?? [];
      expect(Math.hypot(x, y, z), gondola.name).toBeCloseTo(12, 3);
      expect(gondola.mesh, gondola.name).toBeDefined();
    }
  });

  it("paints the wheel in wall white over beam shade and its gondolas two per crowd colour", () => {
    expect(materialsOf(brandsPatch, "Prop.circuit-wheel.1.rim")).toEqual(["beam-shade", "wall"]);
    const worn = new Map<string, number>();
    for (let k = 1; k <= 16; k++) {
      const [hanger, colour] = materialsOf(brandsPatch, `Prop.circuit-wheel.1.gondola.${k}`);
      expect(hanger, `gondola ${k}`).toBe("carbon");
      expect(colour, `gondola ${k}`).toMatch(/^crowd-[0-7]$/);
      worn.set(colour ?? "", (worn.get(colour ?? "") ?? 0) + 1);
    }
    expect([...worn.values()]).toEqual(Array.from({ length: 8 }, () => 2));
  });

  it("spans Dunlop with a tyre arch, its sponsor bent round the belt on both faces", () => {
    for (const part of ["tyre", "tyre-shade", "tyre-face", "kerb-white", "wall", "stand-plinth"]) {
      expect(materialsOf(brandsPatch, `Prop.arch.${part}`), part).toEqual([part]);
    }
    const belts = ["front", "back"].map((face): readonly number[] => {
      const node = brandsPatch.nodes.find(({ name }) => name === `Prop.arch.1.belt.${face}`);
      expect(node?.mesh, face).toBeDefined();
      expect(materialsOf(brandsPatch, `Prop.arch.1.belt.${face}`)).toEqual(["ink-black"]);
      return node?.translation ?? [];
    });
    // One lettering just off each face of the 2 m ring, at the grass, both bent round
    // the same 14.65 m belt so the glyphs sit between its radii.
    const [ax = 0, ay = 0, az = 0] = belts[0] ?? [];
    const [bx = 0, by = 0, bz = 0] = belts[1] ?? [];
    expect(Math.hypot(ax - bx, az - bz)).toBeCloseTo(2 + 2 * 0.06, 3);
    expect(ay).toBeCloseTo(-0.05, 3);
    expect(by).toBeCloseTo(-0.05, 3);
    for (const face of ["front", "back"]) {
      const node = brandsPatch.nodes.find(({ name }) => name === `Prop.arch.1.belt.${face}`);
      const mesh = node?.mesh === undefined ? undefined : brandsPatch.meshes[node.mesh];
      const bounds = brandsPatch.accessors[mesh?.primitives[0]?.attributes.POSITION ?? -1];
      // Text lies in the node's XY plane reading along +X, so the belt's radius is along -Z
      // and the crown is at X zero: the name is centred on it.
      const [left = Number.NaN, , nearest = Number.NaN] = bounds?.max ?? [];
      const [right = Number.NaN, , furthest = Number.NaN] = bounds?.min ?? [];
      expect(-nearest, face).toBeGreaterThan(14.65 - 1.2);
      expect(-furthest, face).toBeLessThan(14.65 + 0.6);
      expect(Math.abs(left + right), face).toBeLessThan(0.5);
    }
  });

  it("faces the grid with a pit building: a door with a lit band and a flag in each crowd colour", () => {
    for (const part of ["stand-riser", "stand-plinth", "wall", "carbon"]) {
      expect(materialsOf(brandsPatch, `Prop.pits.${part}`), part).toEqual([part]);
    }
    for (let slot = 0; slot < 8; slot++) {
      expect(materialsOf(brandsPatch, `Prop.pits.crowd-${slot}`)).toEqual([`crowd-${slot}`]);
      expect(materialsOf(brandsPatch, `Prop.pits.crowd-${slot}-lit`)).toEqual([
        `crowd-${slot}-lit`,
      ]);
      const lit = brandsPatch.materials?.find(({ name }) => name === `crowd-${slot}-lit`);
      const key = brandsPatch.materials?.find(({ name }) => name === `crowd-${slot}`);
      const [lr = 0, lg = 0, lb = 0] = lit?.pbrMetallicRoughness?.baseColorFactor ?? [];
      const [kr = 0, kg = 0, kb = 0] = key?.pbrMetallicRoughness?.baseColorFactor ?? [];
      // The band is the key colour lifted toward white, never darker on any channel.
      expect([lr >= kr, lg >= kg, lb >= kb], `crowd-${slot}`).toEqual([true, true, true]);
      expect(lr + lg + lb, `crowd-${slot}`).toBeGreaterThan(kr + kg + kb);
    }
    // No lettering on the building: the doors are colours only (ADR 0035).
    expect(nodeNames(brandsPatch).filter((name) => name.startsWith("Prop.pits."))).toHaveLength(
      4 + 16,
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
    expect(families).not.toContain("Prop");
  });
});
