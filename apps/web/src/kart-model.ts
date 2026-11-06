// The kart the View draws: a chibi low-poly F1 car (ADR 0034) that
// assets/karts/build.py builds from primitives, painted in the materials
// named here, which the View swaps for the slot's livery (ADR 0035) and
// driver, whose bare head wears only that driver's parts (ADR 0036). On a
// star's run every material is pulled toward the shine (ADR 0075), so each
// kart carries its own copy of every material.
import { type Material, Mesh, MeshStandardMaterial, type Object3D } from "three";
import kartGltf from "../../../assets/karts/kart.glb?url";
import type { KartSize } from "./bridge";
import { FACE_PARTS, partsOf } from "./driver-parts";
import type { Livery } from "./livery";
import { mixColour } from "./star-shine";

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
 * A kart in `livery`: a clone of `model` whose every mesh gets the kart's
 * own copy of its material, the livery and driver materials in that
 * material's colour of the slot and the rest, carbon, tyre and rim, as
 * exported, so the shine of one kart's star never lights another. Of the
 * head's swappable parts, only the slot's driver's stay visible.
 */
export function dressKart(model: Object3D, livery: Livery): Object3D {
  const kart = model.clone();
  const colours = paints(livery);
  const worn = partsOf(livery.driver);
  const own = new Map<Material, Material>();
  kart.traverse((object) => {
    if (FACE_PARTS.has(object.name)) {
      object.visible = worn.has(object.name);
    }
    if (!(object instanceof Mesh) || Array.isArray(object.material)) {
      return;
    }
    const material = object.material;
    const copy = own.get(material) ?? repaint(material, colours.get(material.name));
    own.set(material, copy);
    object.material = copy;
  });
  return kart;
}

/** The copy's own colour, kept so a shine can be pulled toward and let go of. */
const BASE_COLOUR = "baseColour";

/**
 * A copy of `material`, in `colour` when given. The glTF loader makes
 * standard materials; anything else keeps its colour.
 */
function repaint(material: Material, colour: number | undefined): Material {
  const copy = material.clone();
  if (copy instanceof MeshStandardMaterial) {
    if (colour !== undefined) copy.color.setHex(colour);
    copy.userData[BASE_COLOUR] = copy.color.getHex();
  }
  return copy;
}

/**
 * Pull every material of a dressed `kart` `pull` of the way toward
 * `colour` and light it by it as emissive, so its own shading still shows
 * through; with `colour` null, or a pull of nothing, every material is its
 * own colour again. Only a kart `dressKart` made has materials of its own
 * to shine.
 */
export function shineKart(kart: Object3D, colour: number | null, pull: number): void {
  kart.traverse((object) => {
    if (!(object instanceof Mesh) || !(object.material instanceof MeshStandardMaterial)) return;
    const material = object.material;
    const base = material.userData[BASE_COLOUR];
    if (typeof base !== "number") return;
    if (colour === null || pull <= 0) {
      material.color.setHex(base);
      material.emissive.setHex(0);
      return;
    }
    material.color.setHex(mixColour(base, colour, pull));
    material.emissive.setHex(mixColour(0, colour, pull));
  });
}
