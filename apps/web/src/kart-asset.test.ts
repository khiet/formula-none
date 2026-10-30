import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { loadBridge } from "./bridge";
import { type GltfJson, gltfJson } from "./gltf-json";
import { DRIVER_MATERIALS, fitKart, LIVERY_MATERIALS } from "./kart-model";
import { FACIAL_HAIRS, HAIRS } from "./livery";
import { KART_PAINT } from "./palette";
import { WHEELS } from "./wheel-spin";

// The exported kart as the View loads it, read as bytes.
const glb = readFileSync(new URL("../../../assets/karts/kart.glb", import.meta.url));
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));

const kart = gltfJson(glb);

/** Where each node's origin sits in the model: its translation and its parents'. */
function origins(gltf: GltfJson): [number, number, number][] {
  const at = gltf.nodes.map((): [number, number, number] => [0, 0, 0]);
  const place = (index: number, parent: readonly [number, number, number]) => {
    const node = gltf.nodes[index];
    const origin = at[index];
    if (node === undefined || origin === undefined) {
      return;
    }
    for (let axis = 0; axis < 3; axis += 1) {
      origin[axis] = (parent[axis] ?? 0) + (node.translation?.[axis] ?? 0);
    }
    for (const child of node.children ?? []) {
      place(child, origin);
    }
  };
  const children = new Set(gltf.nodes.flatMap((node) => node.children ?? []));
  gltf.nodes.forEach((_, index) => {
    if (!children.has(index)) {
      place(index, [0, 0, 0]);
    }
  });
  return at;
}

/**
 * The whole model's bounds: every node's mesh bounds moved to the node's
 * origin. Every node is placed unrotated and unscaled, so a translation is
 * the only transform there is.
 */
function bounds(gltf: GltfJson) {
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  const at = origins(gltf);
  gltf.nodes.forEach((node, index) => {
    const mesh = node.mesh === undefined ? undefined : gltf.meshes[node.mesh];
    for (const primitive of mesh?.primitives ?? []) {
      const accessor = gltf.accessors[primitive.attributes.POSITION];
      for (let axis = 0; axis < 3; axis += 1) {
        const shift = at[index]?.[axis] ?? 0;
        min[axis] = Math.min(min[axis] ?? Infinity, (accessor?.min?.[axis] ?? Infinity) + shift);
        max[axis] = Math.max(max[axis] ?? -Infinity, (accessor?.max?.[axis] ?? -Infinity) + shift);
      }
    }
  });
  return { min: min as [number, number, number], max: max as [number, number, number] };
}

describe("the exported kart", () => {
  it.each([...Object.values(LIVERY_MATERIALS), ...Object.values(DRIVER_MATERIALS)])(
    "carries one %s material, painted on some face, so the View can swap it",
    (name) => {
      const materials = kart.materials ?? [];
      const index = materials.findIndex((material) => material.name === name);
      expect(materials.filter((material) => material.name === name)).toHaveLength(1);
      const used = kart.meshes.some(({ primitives }) =>
        primitives.some(({ material }) => material === index),
      );
      expect(used).toBe(true);
    },
  );

  it.each(Object.entries(KART_PAINT))(
    "paints its fixed %s as palette.ts copies it, so the poster kart matches the race",
    (name, colour) => {
      const material = (kart.materials ?? []).find(
        (candidate) => candidate.name?.toLowerCase() === name,
      );
      const linear = material?.pbrMetallicRoughness?.baseColorFactor ?? [];
      // The glb stores the colour linear; the palette holds it as the sRGB build.py set.
      const srgb = (channel: number) =>
        Math.round(
          255 * (channel <= 0.0031308 ? channel * 12.92 : 1.055 * channel ** (1 / 2.4) - 0.055),
        );
      expect(linear).toHaveLength(4);
      expect(linear.slice(0, 3).map(srgb)).toEqual(
        [16, 8, 0].map((shift) => (colour >> shift) & 0xff),
      );
    },
  );

  it("races the driver bare-headed: no helmet to paint", () => {
    expect((kart.materials ?? []).map(({ name }) => name)).not.toContain("Helmet");
  });

  it("carries a part for every hair style, every facial hair and the earrings, for the View to pick from", () => {
    const names = kart.nodes.map(({ name }) => name);
    for (const hair of HAIRS) {
      expect(names).toContain(`hair-${hair}`);
    }
    for (const facialHair of FACIAL_HAIRS.filter((facialHair) => facialHair !== "none")) {
      expect(names).toContain(`facial-hair-${facialHair}`);
    }
    expect(names).toContain("earrings");
  });

  it("carries the driver's head as a node of its own, wearing every hair, facial hair and the earrings, for the View to roll", () => {
    const head = kart.nodes.find(({ name }) => name === "head");
    expect(head?.rotation).toBeUndefined();
    const worn = (head?.children ?? []).map((child) => kart.nodes[child]?.name);
    for (const hair of HAIRS) {
      expect(worn).toContain(`hair-${hair}`);
    }
    for (const facialHair of FACIAL_HAIRS.filter((facialHair) => facialHair !== "none")) {
      expect(worn).toContain(`facial-hair-${facialHair}`);
    }
    expect(worn).toContain("earrings");
  });

  it("pivots the head at the neck: on the kart's centreline, under the chin", () => {
    const index = kart.nodes.findIndex(({ name }) => name === "head");
    const head = kart.nodes[index];
    const mesh = head?.mesh === undefined ? undefined : kart.meshes[head.mesh];
    const lowest = Math.min(
      ...(mesh?.primitives ?? []).map(
        ({ attributes }) => kart.accessors[attributes.POSITION]?.min?.[1] ?? Infinity,
      ),
    );
    expect(origins(kart)[index]?.[0]).toBeCloseTo(0, 6);
    expect(lowest).toBeCloseTo(0, 3);
  });

  it("keeps the top of the driver's hair at or below 1.34 m", () => {
    // The height the chase camera and the kart labels are placed to clear.
    expect(bounds(kart).max[1]).toBeLessThanOrEqual(1.34 + 1e-6);
  });

  it("is flat-coloured: no texture to fetch, decode or upload per kart", () => {
    expect(kart.images ?? []).toHaveLength(0);
    expect(kart.textures ?? []).toHaveLength(0);
  });

  it("keeps the four wheels as nodes of their own, for the tells to find", () => {
    const names = kart.nodes.map(({ name }) => name);
    for (const wheel of WHEELS) {
      expect(names).toContain(wheel);
    }
  });

  it.each(WHEELS)(
    "builds %s about its axle, unrotated, for the View to turn it in place",
    (name) => {
      const node = kart.nodes.find((candidate) => candidate.name === name);
      expect(node?.rotation).toBeUndefined();
      const mesh = node?.mesh === undefined ? undefined : kart.meshes[node.mesh];
      const accessors = (mesh?.primitives ?? []).map(
        ({ attributes }) => kart.accessors[attributes.POSITION],
      );
      expect(accessors.length).toBeGreaterThan(0);
      // Up (Y) and along the kart (Z), the wheel's bounds sit evenly about its origin.
      for (const axis of [1, 2]) {
        const low = Math.min(...accessors.map((accessor) => accessor?.min?.[axis] ?? Infinity));
        const high = Math.max(...accessors.map((accessor) => accessor?.max?.[axis] ?? -Infinity));
        expect(low + high).toBeCloseTo(0, 5);
      }
    },
  );

  it("fills the sim's box footprint once fitted, without overhanging it", async () => {
    const { kartSize } = await loadBridge(wasm);
    const { min, max } = bounds(kart);
    const fit = fitKart({ min, max }, kartSize);
    const width = (max[0] - min[0]) * fit.scale;
    const length = (max[2] - min[2]) * fit.scale;
    expect(width).toBeLessThanOrEqual(kartSize.width + 1e-6);
    expect(length).toBeLessThanOrEqual(kartSize.length + 1e-6);
    expect(Math.max(width / kartSize.width, length / kartSize.length)).toBeCloseTo(1, 5);
    // Neither axis is left far short: a kart that fills the box's width also nearly fills its length.
    expect(Math.min(width / kartSize.width, length / kartSize.length)).toBeGreaterThan(0.9);
    // Wheels on the floor of the box: the model's lowest point is its ground.
    expect(min[1]).toBeCloseTo(0, 2);
  });
});
