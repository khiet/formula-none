// The kart the View draws: a chibi low-poly F1 car (ADR 0034) that
// assets/karts/build.py builds from primitives, painted in four materials
// named here, which the View swaps for the slot's livery (ADR 0035).
import { type Material, Mesh, MeshStandardMaterial, type Object3D } from "three";
import kartGltf from "../../../assets/karts/kart.glb?url";
import type { KartSize } from "./bridge";
import type { Livery } from "./livery";

/** The exported kart, as the View fetches it. */
export const KART_GLTF_URL: string = kartGltf;

type Paint = "primary" | "secondary" | "accent" | "helmet";

/** The paint materials in the exported kart, by the livery colour every face in each takes. */
export const LIVERY_MATERIALS: Readonly<Record<Paint, string>> = {
  primary: "Primary",
  secondary: "Secondary",
  accent: "Accent",
  helmet: "Helmet",
};

const PAINT_BY_MATERIAL = new Map(
  Object.entries(LIVERY_MATERIALS).map(([paint, name]) => [name, paint as Paint]),
);

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
 * driver's helmet may stand above the box.
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
 * A kart in `livery`: a clone of `model` whose meshes wearing a livery
 * material get a copy of it in that material's colour of the palette, so
 * each kart on the grid has its own paint and every other material stays
 * shared across them.
 */
export function dressKart(model: Object3D, livery: Livery): Object3D {
  const kart = model.clone();
  const own = new Map<Material, Material>();
  kart.traverse((object) => {
    if (!(object instanceof Mesh) || Array.isArray(object.material)) {
      return;
    }
    const material = object.material;
    const paint = PAINT_BY_MATERIAL.get(material.name);
    if (paint === undefined) {
      return;
    }
    const copy = own.get(material) ?? repaint(material, livery[paint]);
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
