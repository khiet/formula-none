import { type Material, Mesh, type Object3D } from "three";
import { describe, expect, it } from "vitest";
import { hazardConeMesh, homingGullMesh, itemBoxMesh } from "./item-meshes";

function materialsOf(object: Object3D): Material[] {
  const found: Material[] = [];
  object.traverse((child) => {
    if (child instanceof Mesh) found.push(...[child.material].flat());
  });
  return found;
}

function coloursOf(object: Object3D): number[] {
  return materialsOf(object).map((material) =>
    "color" in material && typeof material.color === "object" && material.color !== null
      ? (material.color as { getHex(): number }).getHex()
      : -1,
  );
}

describe("the item meshes", () => {
  it.each([
    ["item box", itemBoxMesh],
    ["hazard cone", hazardConeMesh],
    ["homing gull", homingGullMesh],
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
});
