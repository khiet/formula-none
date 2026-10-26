import { BufferGeometry, Group, Mesh, MeshStandardMaterial, type Object3D } from "three";
import { describe, expect, it } from "vitest";
import { dressKart, fitKart, LIVERY_MATERIALS } from "./kart-model";
import { livery } from "./livery";

const SIZE = { width: 1.2, height: 0.6, length: 1.8 };

/** A stand-in for the exported car: one mesh per livery material, however deep, dark tyres. */
function model(): Group {
  const root = new Group();
  const paint = (name: string) => new MeshStandardMaterial({ name, color: 0x808080 });
  const part = (name: string, material: MeshStandardMaterial) => {
    const mesh = new Mesh(new BufferGeometry(), material);
    mesh.name = name;
    return mesh;
  };
  const primary = paint(LIVERY_MATERIALS.primary);
  const driver = new Group();
  driver.add(part("helmet", paint(LIVERY_MATERIALS.helmet)));
  root.add(
    part("body", primary),
    part("nose", primary),
    part("sidepod", paint(LIVERY_MATERIALS.secondary)),
    part("wing", paint(LIVERY_MATERIALS.accent)),
    part("wheel-front-left", new MeshStandardMaterial({ name: "Tyre", color: 0x202020 })),
    driver,
  );
  return root;
}

function colourOf(root: Object3D, name: string): number {
  return materialOf(root, name).color.getHex();
}

function materialOf(root: Object3D, name: string): MeshStandardMaterial {
  const mesh = root.getObjectByName(name);
  if (!(mesh instanceof Mesh)) throw new Error(`${name} is not a mesh`);
  return mesh.material as MeshStandardMaterial;
}

describe("dressing a kart", () => {
  it("paints each livery material in its colour of the palette, and leaves the tyres alone", () => {
    const notte = livery(1);
    const kart = dressKart(model(), notte);
    expect(colourOf(kart, "body")).toBe(notte.primary);
    expect(colourOf(kart, "nose")).toBe(notte.primary);
    expect(colourOf(kart, "sidepod")).toBe(notte.secondary);
    expect(colourOf(kart, "wing")).toBe(notte.accent);
    expect(colourOf(kart, "helmet")).toBe(notte.helmet);
    expect(colourOf(kart, "wheel-front-left")).toBe(0x202020);
  });

  it("gives each kart its own paint, so eight karts from one model wear eight liveries", () => {
    const source = model();
    const rosso = dressKart(source, livery(0));
    const papaya = dressKart(source, livery(3));
    expect(colourOf(rosso, "body")).toBe(livery(0).primary);
    expect(colourOf(papaya, "body")).toBe(livery(3).primary);
    expect(colourOf(source, "body")).toBe(0x808080);
    // Meshes sharing a livery material share the kart's copy of it.
    expect(materialOf(rosso, "body")).toBe(materialOf(rosso, "nose"));
    // The unpainted parts stay shared: one tyre material for the whole grid.
    expect(materialOf(rosso, "wheel-front-left")).toBe(materialOf(papaya, "wheel-front-left"));
  });
});

describe("fitting a kart", () => {
  // A kart's bounds, metres, Y up, with the wheels resting on y = 0
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
