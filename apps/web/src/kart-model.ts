// The kart the View draws: a chibi low-poly F1 car (ADR 0034) that
// assets/karts/build.py builds from primitives, painted in the materials
// named here, which the View swaps for the slot's livery (ADR 0035) and
// driver, whose bare head wears only that driver's parts (ADR 0036).
import { type Material, Mesh, MeshStandardMaterial, type Object3D } from "three";
import kartGltf from "../../../assets/karts/kart.glb?url";
import type { KartSize } from "./bridge";
import { type Driver, FACIAL_HAIRS, HAIRS, type Livery } from "./livery";

/** The exported kart, as the View fetches it. */
export const KART_GLTF_URL: string = kartGltf;

type Paint = "primary" | "secondary" | "accent";
type DriverPaint = "skin" | "hairColour";

/** The paint materials in the exported kart, by the livery colour every face in each takes. */
export const LIVERY_MATERIALS: Readonly<Record<Paint, string>> = {
  primary: "Primary",
  secondary: "Secondary",
  accent: "Accent",
};

/** The driver's materials in the exported kart, by the driver's colour each takes. */
export const DRIVER_MATERIALS: Readonly<Record<DriverPaint, string>> = {
  skin: "Skin",
  hairColour: "Hair",
};

/** The colour each painted material takes for `livery`, by material name. */
function paints(livery: Livery): Map<string, number> {
  return new Map([
    ...Object.entries(LIVERY_MATERIALS).map(
      ([paint, name]) => [name, livery[paint as Paint]] as const,
    ),
    ...Object.entries(DRIVER_MATERIALS).map(
      ([paint, name]) => [name, livery.driver[paint as DriverPaint]] as const,
    ),
  ]);
}

/** Every swappable part on the head, by the name the exporter gives its node. */
const DRIVER_PARTS: ReadonlySet<string> = new Set([
  ...HAIRS.map((hair) => `hair-${hair}`),
  ...FACIAL_HAIRS.filter((facialHair) => facialHair !== "none").map(
    (facialHair) => `facial-hair-${facialHair}`,
  ),
  "earrings",
]);

/** The parts `driver` wears; a clean-shaven driver's "none" matches no part, so no facial hair shows. */
function partsOf(driver: Driver): ReadonlySet<string> {
  return new Set([
    `hair-${driver.hair}`,
    `facial-hair-${driver.facialHair}`,
    ...(driver.earrings ? ["earrings"] : []),
  ]);
}

export interface Bounds {
  readonly min: readonly [number, number, number];
  readonly max: readonly [number, number, number];
}

/** How the model is placed inside the sim's kart box. */
export interface KartFit {
  /** Uniform; the model fills the box's footprint along its length or its width, whichever binds first. */
  readonly scale: number;
  /** Where the model's origin goes along the kart's length so its middle is the box's. */
  readonly centreZ: number;
  /** Where the model's origin goes vertically so its lowest point rests on the box's floor. */
  readonly groundY: number;
}

/**
 * Fit a model, by its bounds, into the box the sim collides with: scaled
 * until its footprint meets the box's length or width, standing on the
 * floor, centred along the length. The sim's transform is the box's centre,
 * so the floor is half a height below it. Height is not fitted: the
 * driver's head may stand above the box.
 */
export function fitKart(bounds: Bounds, size: KartSize): KartFit {
  const scale = Math.min(
    size.length / (bounds.max[2] - bounds.min[2]),
    size.width / (bounds.max[0] - bounds.min[0]),
  );
  const middleZ = (bounds.max[2] + bounds.min[2]) / 2;
  return {
    scale,
    centreZ: -middleZ * scale,
    groundY: -size.height / 2 - bounds.min[1] * scale,
  };
}

/**
 * A kart in `livery`: a clone of `model` whose meshes wearing a livery or
 * driver material get a copy of it in that material's colour of the slot,
 * so each kart on the grid has its own paint and every other material stays
 * shared across them. Of the head's swappable parts, only the slot's
 * driver's stay visible.
 */
export function dressKart(model: Object3D, livery: Livery): Object3D {
  const kart = model.clone();
  const colours = paints(livery);
  const worn = partsOf(livery.driver);
  const own = new Map<Material, Material>();
  kart.traverse((object) => {
    if (DRIVER_PARTS.has(object.name)) {
      object.visible = worn.has(object.name);
    }
    if (!(object instanceof Mesh) || Array.isArray(object.material)) {
      return;
    }
    const material = object.material;
    const colour = colours.get(material.name);
    if (colour === undefined) {
      return;
    }
    const copy = own.get(material) ?? repaint(material, colour);
    own.set(material, copy);
    object.material = copy;
  });
  return kart;
}

/** A copy of `material` in `colour`. The glTF loader makes standard materials; anything else keeps its colour. */
function repaint(material: Material, colour: number): Material {
  const copy = material.clone();
  if (copy instanceof MeshStandardMaterial) {
    copy.color.setHex(colour);
  }
  return copy;
}
