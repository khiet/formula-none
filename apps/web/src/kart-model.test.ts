import { BufferGeometry, Group, Mesh, MeshStandardMaterial, type Object3D } from "three";
import { describe, expect, it } from "vitest";
import { dressKart, fitKart, LIVERY_MATERIAL } from "./kart-model";

const SIZE = { width: 1.2, height: 0.6, length: 1.8 };

/** A stand-in for the exported kart: a painted body, dark tyres and a driver. */
function model(): Group {
  const root = new Group();
  const paint = new MeshStandardMaterial({ name: LIVERY_MATERIAL, color: 0x808080 });
  const rubber = new MeshStandardMaterial({ name: "Tyre", color: 0x202020 });
  const body = new Mesh(new BufferGeometry(), paint);
  body.name = "body";
  const wheel = new Mesh(new BufferGeometry(), rubber);
  wheel.name = "wheel-front-left";
  const driver = new Group();
  const suit = new Mesh(new BufferGeometry(), paint);
  suit.name = "suit";
  driver.add(suit);
  root.add(body, wheel, driver);
  return root;
}

function materialOf(root: Object3D, name: string): MeshStandardMaterial {
  const mesh = root.getObjectByName(name);
  if (!(mesh instanceof Mesh)) throw new Error(`${name} is not a mesh`);
  return mesh.material as MeshStandardMaterial;
}

describe("dressing a kart", () => {
  it("paints every mesh wearing the livery material, however deep, in the kart's colour", () => {
    const kart = dressKart(model(), 0xd62828);
    expect(materialOf(kart, "body").color.getHex()).toBe(0xd62828);
    expect(materialOf(kart, "suit").color.getHex()).toBe(0xd62828);
    expect(materialOf(kart, "wheel-front-left").color.getHex()).toBe(0x202020);
  });

  it("gives each kart its own paint, so eight karts from one model wear eight liveries", () => {
    const source = model();
    const red = dressKart(source, 0xd62828);
    const blue = dressKart(source, 0x1f5fd1);
    expect(materialOf(red, "body").color.getHex()).toBe(0xd62828);
    expect(materialOf(blue, "body").color.getHex()).toBe(0x1f5fd1);
    expect(materialOf(source, "body").color.getHex()).toBe(0x808080);
    // The unpainted parts stay shared: one tyre material for the whole grid.
    expect(materialOf(red, "wheel-front-left")).toBe(materialOf(blue, "wheel-front-left"));
  });
});

describe("fitting a kart", () => {
  // The Kenney kart's bounds, metres, Y up, with the wheels resting on y = 0
  // and the driver's helmet the highest point.
  const bounds = { min: [-0.47, 0, -0.6] as const, max: [0.47, 1.05, 0.83] as const };

  it("scales the model until its length meets the box's and stands it on the floor", () => {
    const fit = fitKart(bounds, SIZE);
    expect(fit.scale).toBeCloseTo(1.8 / 1.43, 5);
    // The sim's transform is the box's centre; the model's y = 0 lands at the floor.
    expect(fit.groundY).toBeCloseTo(-0.3, 5);
  });

  it("lets the width bind when the model is squarer than the box", () => {
    const fit = fitKart({ min: [-1, 0, -1], max: [1, 1, 1] }, SIZE);
    expect(fit.scale).toBeCloseTo(0.6, 5);
  });

  it("centres a model whose length is not centred on its origin", () => {
    const fit = fitKart({ min: [-0.5, 0.1, 0], max: [0.5, 1, 2] }, SIZE);
    expect(fit.scale).toBeCloseTo(0.9, 5);
    expect(fit.centreZ).toBeCloseTo(-0.9, 5);
    expect(fit.groundY).toBeCloseTo(-0.3 - 0.1 * 0.9, 5);
  });
});
