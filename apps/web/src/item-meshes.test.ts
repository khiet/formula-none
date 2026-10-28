import { Box3, type Material, Mesh, MeshLambertMaterial, type Object3D, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import {
  hazardConeMesh,
  homingGullMesh,
  itemBoxMesh,
  marshalFlagMesh,
  runawayWheelBounce,
  runawayWheelMesh,
} from "./item-meshes";
import { KART_RIM, KART_TYRE, SIGNAL_YELLOW } from "./palette";

/** A runaway wheel's radius, any will do. */
const RADIUS_M = 0.45;

function materialsOf(object: Object3D): Material[] {
  const found: Material[] = [];
  object.traverse((child) => {
    if (child instanceof Mesh) found.push(...[child.material].flat());
  });
  return found;
}

function coloursOf(object: Object3D): number[] {
  return materialsOf(object).flatMap((material) =>
    material instanceof MeshLambertMaterial ? [material.color.getHex()] : [],
  );
}

describe("the item meshes", () => {
  it.each([
    ["item box", itemBoxMesh],
    ["hazard cone", hazardConeMesh],
    ["homing gull", homingGullMesh],
    ["runaway wheel", () => runawayWheelMesh(RADIUS_M)],
    ["marshal's flag", marshalFlagMesh],
  ])("draw the %s lit by the sun and flat-shaded, facet by facet", (_, mesh) => {
    const materials = materialsOf(mesh());
    expect(materials.length).toBeGreaterThan(0);
    for (const material of materials) {
      expect("isMeshBasicMaterial" in material).toBe(false);
      expect(material).toHaveProperty("flatShading", true);
    }
  });

  it("paint the item box in the item slot's golds", () => {
    expect(coloursOf(itemBoxMesh())).toEqual(expect.arrayContaining([0xf8e39a, 0xe0b54a]));
  });

  it("paint the cone hazard orange with a white band and a grey slab", () => {
    expect(coloursOf(hazardConeMesh())).toEqual(
      expect.arrayContaining([0xff5522, 0xffffff, 0x9aa0a8]),
    );
  });

  it("paint the gull white with a grey wing, an orange beak and a red eye", () => {
    expect(coloursOf(homingGullMesh())).toEqual(
      expect.arrayContaining([0xffffff, 0xb9bec5, 0xff5522, 0xd00a12]),
    );
  });

  it("paint the runaway wheel in the kart's tyre and rim", () => {
    expect(coloursOf(runawayWheelMesh(RADIUS_M))).toEqual(
      expect.arrayContaining([KART_TYRE, KART_RIM]),
    );
  });

  it("draw the runaway wheel as big as it is asked for, its axle across its roll", () => {
    const size = new Box3().setFromObject(runawayWheelMesh(RADIUS_M)).getSize(new Vector3());
    // The spokes stand a hair proud of the tyre's faces.
    expect(size.y).toBeCloseTo(2 * RADIUS_M, 1);
    expect(size.z).toBeCloseTo(2 * RADIUS_M, 1);
    expect(size.x).toBeLessThan(size.z);
  });

  it("bounce the runaway wheel three times a pass, down at each wall", () => {
    for (const down of [0, 1 / 3, 2 / 3, 1]) {
      expect(runawayWheelBounce(down)).toBeCloseTo(0, 6);
    }
    for (const up of [1 / 6, 1 / 2, 5 / 6]) {
      expect(runawayWheelBounce(up)).toBeGreaterThan(0.3);
    }
  });

  it("fly the marshal's flag in signal yellow from the top of its pole", () => {
    const flag = marshalFlagMesh();
    expect(coloursOf(flag)).toContain(SIGNAL_YELLOW.mid);
    const bounds = new Box3().setFromObject(flag);
    expect(bounds.min.y).toBeCloseTo(0, 6);
    expect(bounds.min.x).toBeLessThan(0);
    expect(bounds.max.x).toBeGreaterThan(0.5);
  });
});
