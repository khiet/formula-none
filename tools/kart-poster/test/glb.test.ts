import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseGlb } from "../src/glb.ts";

const kart = parseGlb(
  new Uint8Array(readFileSync(new URL("../../../assets/karts/kart.glb", import.meta.url))),
);

/** A glb of one triangle under `node`, for the contracts the real export never exercises. */
function glbOf(node: Record<string, unknown>): Uint8Array {
  const positions = new Float32Array([0, 0, 0, 0, 0, -1, 0, 1, -1]);
  const indices = new Uint16Array([0, 1, 2, 0]);
  const bin = new Uint8Array(positions.byteLength + indices.byteLength);
  bin.set(new Uint8Array(positions.buffer), 0);
  bin.set(new Uint8Array(indices.buffer), positions.byteLength);
  const text = JSON.stringify({
    asset: { version: "2.0" },
    scenes: [{ nodes: [0] }],
    nodes: [{ name: "part", mesh: 0, ...node }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, material: 0 }] }],
    materials: [{ name: "Primary" }],
    accessors: [
      { bufferView: 0, componentType: 5126, count: 3, type: "VEC3" },
      { bufferView: 1, componentType: 5123, count: 3, type: "SCALAR" },
    ],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: positions.byteLength },
      { buffer: 0, byteOffset: positions.byteLength, byteLength: indices.byteLength },
    ],
    buffers: [{ byteLength: bin.byteLength }],
  });
  const json = new TextEncoder().encode(text.padEnd(Math.ceil(text.length / 4) * 4, " "));
  const out = new Uint8Array(12 + 8 + json.byteLength + 8 + bin.byteLength);
  const view = new DataView(out.buffer);
  out.set(new TextEncoder().encode("glTF"), 0);
  view.setUint32(4, 2, true);
  view.setUint32(8, out.byteLength, true);
  view.setUint32(12, json.byteLength, true);
  out.set(new TextEncoder().encode("JSON"), 16);
  out.set(json, 20);
  view.setUint32(20 + json.byteLength, bin.byteLength, true);
  out.set(new TextEncoder().encode("BIN\0"), 24 + json.byteLength);
  out.set(bin, 28 + json.byteLength);
  return out;
}

describe("parseGlb", () => {
  it("reads every meshed part of kart.glb with the material each face wears", () => {
    expect(kart.meshed).toContain("head");
    expect(kart.meshed).toContain("wheel-front-left");
    expect(kart.meshed).toContain("hair-short-curls");
    const materials = (node: string) =>
      new Set(kart.triangles.filter((t) => t.node === node).map((t) => t.material));
    expect(materials("wheel-front-left")).toEqual(new Set(["Tyre", "Rim"]));
    expect(materials("head")).toEqual(new Set(["Skin", "Whites", "Irises"]));
    // build.py: Tyre is sRGB (0.11, 0.11, 0.12); an unpainted material is mid grey.
    expect(kart.materials.get("Tyre")).toBe("#1c1c1f");
    expect(kart.materials.get("Primary")).toBe("#808080");
    expect(kart.triangles.find((t) => t.node === "head")?.normal).toHaveLength(3);
  });

  it("places a part by its own and its parents' translations: the head pivots at the neck, in the kart's frame", () => {
    // build.py: NECK is (0, -0.1, 0.97 - 0.3 * 0.78) in Blender's frame, which the export turns Y up, nose along -Z.
    const [x, y, z] = kart.origins.get("head") ?? [NaN, NaN, NaN];
    expect(x).toBeCloseTo(0, 5);
    expect(y).toBeCloseTo(0.736, 5);
    expect(z).toBeCloseTo(0.1, 5);
    const { origins, triangles } = parseGlb(glbOf({ translation: [1, 2, 3] }));
    expect(origins.get("part")).toEqual([1, 2, 3]);
    expect(triangles[0]?.corners[2]).toEqual([1, 3, 2]);
  });

  it("refuses a rotated or scaled node rather than draw it in the wrong place", () => {
    expect(() => parseGlb(glbOf({ rotation: [0, 0, 0, 1] }))).toThrow(/rotated or scaled/);
    expect(() => parseGlb(glbOf({ scale: [2, 2, 2] }))).toThrow(/rotated or scaled/);
  });
});
