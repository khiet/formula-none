import { BufferGeometry, Group, Mesh, MeshStandardMaterial, type Object3D } from "three";
import { describe, expect, it } from "vitest";
import { DRIVER_MATERIALS, dressKart, fitKart, LIVERY_MATERIALS } from "./kart-model";
import { livery } from "./livery";

const SIZE = { width: 1.2, height: 0.6, length: 1.8 };

/**
 * A stand-in for the exported kart: one mesh per livery material, however
 * deep, dark tyres, and a head with a few of the driver parts, one of them
 * in two materials the way the exporter makes a group of meshes.
 */
function model(): Group {
  const root = new Group();
  const paint = (name: string) => new MeshStandardMaterial({ name, color: 0x808080 });
  const part = (name: string, material: MeshStandardMaterial) => {
    const mesh = new Mesh(new BufferGeometry(), material);
    mesh.name = name;
    return mesh;
  };
  const primary = paint(LIVERY_MATERIALS.primary);
  const hair = paint(DRIVER_MATERIALS.hairColour);
  const greyingTemples = new Group();
  greyingTemples.name = "hair-short-greying-temples";
  greyingTemples.add(
    part("greying-temples-hair", hair),
    part("greying-temples-grey", paint("Grey")),
  );
  const driver = new Group();
  driver.add(
    part("head", paint(DRIVER_MATERIALS.skin)),
    part("hair-braids-tied-back", hair),
    part("hair-short", hair),
    greyingTemples,
    part("facial-hair-short-beard", hair),
    part("facial-hair-stubble", hair),
    part("earrings", paint("Gold")),
  );
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
    expect(colourOf(kart, "wheel-front-left")).toBe(0x202020);
  });

  it("paints the driver's skin and hair in the slot's driver's colours", () => {
    const rosso = livery(0);
    const kart = dressKart(model(), rosso);
    expect(colourOf(kart, "head")).toBe(rosso.driver.skin);
    expect(colourOf(kart, "hair-braids-tied-back")).toBe(rosso.driver.hairColour);
    expect(colourOf(kart, "facial-hair-short-beard")).toBe(rosso.driver.hairColour);
  });

  it("shows only the slot's driver's hair, facial hair and earrings", () => {
    const shown = (kart: Object3D) =>
      [
        "hair-braids-tied-back",
        "hair-short",
        "hair-short-greying-temples",
        "facial-hair-short-beard",
        "facial-hair-stubble",
        "earrings",
      ].filter((name) => kart.getObjectByName(name)?.visible);
    const source = model();
    // Rosso's driver: braids tied back, a short beard and earrings.
    expect(shown(dressKart(source, livery(0)))).toEqual([
      "hair-braids-tied-back",
      "facial-hair-short-beard",
      "earrings",
    ]);
    // Verde's: short with greying temples and stubble, no earrings.
    expect(shown(dressKart(source, livery(4)))).toEqual([
      "hair-short-greying-temples",
      "facial-hair-stubble",
    ]);
    // Notte's facial hair is light stubble, which this stand-in lacks; short hair is all that shows.
    expect(shown(dressKart(source, livery(1)))).toEqual(["hair-short"]);
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
  // and the driver's head the highest point.
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
