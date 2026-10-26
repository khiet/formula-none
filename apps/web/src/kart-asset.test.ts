import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { loadBridge } from "./bridge";
import { type GltfJson, gltfJson } from "./gltf-json";
import { DRIVER_MATERIALS, fitKart, LIVERY_MATERIALS } from "./kart-model";
import { FACIAL_HAIRS, HAIRS } from "./livery";

// The exported kart as the View loads it, read as bytes.
const glb = readFileSync(new URL("../../../assets/karts/kart.glb", import.meta.url));
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));

const kart = gltfJson(glb);

/**
 * The whole model's bounds: every node's mesh bounds moved by the node's
 * translation. The wheels are nodes placed at their axles, unrotated and
 * unscaled, so a translation is the only transform there is.
 */
function bounds(gltf: GltfJson) {
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (const node of gltf.nodes) {
    const mesh = node.mesh === undefined ? undefined : gltf.meshes[node.mesh];
    for (const primitive of mesh?.primitives ?? []) {
      const accessor = gltf.accessors[primitive.attributes.POSITION];
      for (let axis = 0; axis < 3; axis += 1) {
        const shift = node.translation?.[axis] ?? 0;
        min[axis] = Math.min(min[axis] ?? Infinity, (accessor?.min?.[axis] ?? Infinity) + shift);
        max[axis] = Math.max(max[axis] ?? -Infinity, (accessor?.max?.[axis] ?? -Infinity) + shift);
      }
    }
  }
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
    for (const wheel of ["front-left", "front-right", "back-left", "back-right"]) {
      expect(names).toContain(`wheel-${wheel}`);
    }
  });

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
