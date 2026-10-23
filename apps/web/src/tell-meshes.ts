// The Three.js side of the tells: one mesh per tell, sized once to its
// store's capacity and rewritten in place each frame, so eight karts' marks
// are one draw call and each particle pool another, and the scene graph
// never churns.
// Presentation only; everything drawn here was decided by a tell module
// from the transforms the bridge reports.

import {
  BufferAttribute,
  BufferGeometry,
  CircleGeometry,
  Color,
  DoubleSide,
  InstancedMesh,
  type Material,
  Matrix4,
  Mesh,
  type Object3D,
  Vector3,
} from "three";
import { attribute, float, smoothstep, uv } from "three/tsl";
import { MeshBasicNodeMaterial } from "three/webgpu";
import type { KartSize } from "./bridge";
import { TYRE_WIDTH_M } from "./kart-frame";
import type { KartShadows } from "./kart-shadows";
import type { Particles } from "./particles";
import type { TyreMarks } from "./tyre-marks";

export interface TellMesh {
  readonly mesh: Object3D;
  /** Redraw from the store's current contents. */
  update(): void;
}

/** Rubber over asphalt: dark, and never fully opaque so the road shows through. */
const MARK_COLOUR = 0x1a1a1a;
const MARK_OPACITY = 0.6;
/** Marks sit this far over the road so they do not fight the road's own faces for depth. */
const MARK_LIFT_M = 0.02;
const VERTICES_PER_MARK = 4;
const FLOATS_PER_VERTEX = 3;

/**
 * Every tyre mark as a quad of the tyre's width, faded by its strength.
 * The quad's corners are rewritten, and uploaded, only for a mark laid
 * since the last frame; the strength of every live mark is uploaded per
 * frame.
 */
export function tyreMarkMesh(marks: TyreMarks): TellMesh {
  const geometry = new BufferGeometry();
  const corners = new Float32Array(marks.capacity * VERTICES_PER_MARK * FLOATS_PER_VERTEX);
  const strengths = new Float32Array(marks.capacity * VERTICES_PER_MARK);
  const cornerAttribute = new BufferAttribute(corners, FLOATS_PER_VERTEX);
  const strengthAttribute = new BufferAttribute(strengths, 1);
  geometry.setAttribute("position", cornerAttribute);
  geometry.setAttribute("strength", strengthAttribute);
  const indices = new Uint32Array(marks.capacity * 6);
  for (let slot = 0; slot < marks.capacity; slot++) {
    const corner = slot * VERTICES_PER_MARK;
    indices.set([corner, corner + 1, corner + 2, corner + 2, corner + 1, corner + 3], slot * 6);
  }
  geometry.setIndex(new BufferAttribute(indices, 1));
  // Not culled: a bounding sphere over marks spread round the track would
  // exclude nothing and would need recomputing every frame.
  geometry.boundingSphere = null;
  const material = new MeshBasicNodeMaterial();
  material.color = new Color(MARK_COLOUR);
  material.transparent = true;
  material.depthWrite = false;
  material.side = DoubleSide;
  material.opacityNode = attribute("strength", "float").mul(MARK_OPACITY);
  const mesh = new Mesh(geometry, material);
  mesh.frustumCulled = false;
  return {
    mesh,
    update() {
      strengths.fill(0);
      marks.forEach((mark, slot) => {
        const at = slot * VERTICES_PER_MARK * FLOATS_PER_VERTEX;
        const [fx, fy, fz] = mark.from;
        const [tx, ty, tz] = mark.to;
        // Across the mark is the horizontal normal to its run; a tyre's half width each way.
        const dx = tx - fx;
        const dz = tz - fz;
        const run = Math.hypot(dx, dz) || 1;
        const ax = (-dz / run) * (TYRE_WIDTH_M / 2);
        const az = (dx / run) * (TYRE_WIDTH_M / 2);
        // The corners are stored as float32, so the comparison rounds the
        // same way or every live mark would count as changed every frame.
        const changed =
          corners[at] !== Math.fround(fx - ax) ||
          corners[at + 2] !== Math.fround(fz - az) ||
          corners[at + 9] !== Math.fround(tx + ax);
        if (changed) {
          corners[at] = fx - ax;
          corners[at + 1] = fy + MARK_LIFT_M;
          corners[at + 2] = fz - az;
          corners[at + 3] = fx + ax;
          corners[at + 4] = fy + MARK_LIFT_M;
          corners[at + 5] = fz + az;
          corners[at + 6] = tx - ax;
          corners[at + 7] = ty + MARK_LIFT_M;
          corners[at + 8] = tz - az;
          corners[at + 9] = tx + ax;
          corners[at + 10] = ty + MARK_LIFT_M;
          corners[at + 11] = tz + az;
          cornerAttribute.addUpdateRange(at, VERTICES_PER_MARK * FLOATS_PER_VERTEX);
          cornerAttribute.needsUpdate = true;
        }
        strengths.fill(mark.strength, slot * VERTICES_PER_MARK, (slot + 1) * VERTICES_PER_MARK);
      });
      strengthAttribute.needsUpdate = true;
    },
  };
}

/**
 * Every live particle as one instance of `geometry` in `material`, placed
 * at the particle, scaled to `scaleOf` its size and life, in its colour.
 * Dead slots are not drawn: the instance count is the live count.
 */
export function particleMesh(
  particles: Particles,
  geometry: BufferGeometry,
  material: Material,
  scaleOf: (size: number, life: number) => number,
): TellMesh {
  const mesh = new InstancedMesh(geometry, material, particles.capacity);
  mesh.frustumCulled = false;
  // Colour every instance once so the colour buffer exists to rewrite.
  const colour = new Color();
  for (let instance = 0; instance < particles.capacity; instance++) {
    mesh.setColorAt(instance, colour);
  }
  const placement = new Matrix4();
  return {
    mesh,
    update() {
      let live = 0;
      particles.forEach((particle) => {
        const scale = scaleOf(particle.size, particle.life);
        placement.makeScale(scale, scale, scale);
        placement.setPosition(particle.position[0], particle.position[1], particle.position[2]);
        mesh.setMatrixAt(live, placement);
        mesh.setColorAt(live, colour.setHex(particle.colour));
        live += 1;
      });
      mesh.count = live;
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor !== null) {
        mesh.instanceColor.needsUpdate = true;
      }
    },
  };
}

/** A shadow disc: dark and see-through, the same whatever the kart's height. */
const SHADOW_COLOUR = 0x000000;
const SHADOW_OPACITY = 0.8;
/** The disc is full strength out to this share of its radius and fades to nothing at the edge. */
const SHADOW_SOFT_FROM = 0.8;
/**
 * The disc is this much wider than the kart's box each way: the chase
 * camera looks down on the kart's roof, which covers the footprint itself,
 * so the disc has to show round the kart's sides and tail to be seen.
 */
const SHADOW_SPREAD = 1.5;
/** Discs sit this far over the surface so they do not fight its faces for depth. */
const SHADOW_LIFT_M = 0.03;
const SHADOW_SEGMENTS = 32;

/**
 * Every visible shadow disc as one instance of a circle a little wider
 * than the kart's footprint, laid on the surface along its normal and
 * turned to the kart's heading. Constant size and opacity: the height of a
 * hop is read from the gap between kart and disc, not from the disc
 * changing.
 */
export function shadowDiscMesh(shadows: KartShadows, size: KartSize): TellMesh {
  // A unit circle facing +Y; each instance scales it to the disc's size.
  const geometry = new CircleGeometry(0.5, SHADOW_SEGMENTS).rotateX(-Math.PI / 2);
  const material = new MeshBasicNodeMaterial();
  material.color = new Color(SHADOW_COLOUR);
  material.transparent = true;
  material.depthWrite = false;
  // The circle's UV runs 0 to 1 across it, so its centre is half a unit from the edge.
  material.opacityNode = float(SHADOW_OPACITY).mul(
    smoothstep(SHADOW_SOFT_FROM / 2, 0.5, uv().sub(0.5).length()).oneMinus(),
  );
  const mesh = new InstancedMesh(geometry, material, shadows.discs.length);
  mesh.frustumCulled = false;
  const placement = new Matrix4();
  const up = new Vector3();
  const forward = new Vector3();
  const right = new Vector3();
  const back = new Vector3();
  return {
    mesh,
    update() {
      let live = 0;
      for (const disc of shadows.discs) {
        if (!disc.visible) {
          continue;
        }
        up.set(...disc.normal);
        // The heading flattened onto the surface, so the disc lies along it.
        forward.set(...disc.forward).addScaledVector(up, -forward.dot(up));
        if (forward.lengthSq() < 1e-6) {
          continue;
        }
        forward.normalize();
        right.crossVectors(forward, up);
        back.copy(forward).negate();
        placement
          .makeBasis(
            right.multiplyScalar(size.width * SHADOW_SPREAD),
            up,
            back.multiplyScalar(size.length * SHADOW_SPREAD),
          )
          .setPosition(
            disc.position[0] + disc.normal[0] * SHADOW_LIFT_M,
            disc.position[1] + disc.normal[1] * SHADOW_LIFT_M,
            disc.position[2] + disc.normal[2] * SHADOW_LIFT_M,
          );
        mesh.setMatrixAt(live, placement);
        live += 1;
      }
      mesh.count = live;
      mesh.instanceMatrix.needsUpdate = true;
    },
  };
}
