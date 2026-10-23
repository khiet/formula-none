// The Three.js side of the tells: one mesh per tell, sized once to its
// store's capacity and rewritten in place each frame, so eight karts' marks
// and particles are two draw calls each and the scene graph never churns.
// Presentation only; everything drawn here was decided by a tell module
// from the transforms the bridge reports.

import {
  BufferAttribute,
  BufferGeometry,
  Color,
  DoubleSide,
  InstancedMesh,
  type Material,
  Matrix4,
  Mesh,
  type Object3D,
} from "three";
import { attribute } from "three/tsl";
import { MeshBasicNodeMaterial } from "three/webgpu";
import { TYRE_WIDTH_M } from "./kart-frame";
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
 * The quad's corners are rewritten only for a mark that has changed since
 * the last frame, which is the one laid this frame, and only the strength
 * of every live mark is uploaded per frame.
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
  // Never culled: the quads span the track and a kart's view is always among them.
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
        const changed =
          corners[at] !== fx - ax || corners[at + 2] !== fz - az || corners[at + 9] !== tx + ax;
        if (changed) {
          corners.set(
            [
              fx - ax,
              fy + MARK_LIFT_M,
              fz - az,
              fx + ax,
              fy + MARK_LIFT_M,
              fz + az,
              tx - ax,
              ty + MARK_LIFT_M,
              tz - az,
              tx + ax,
              ty + MARK_LIFT_M,
              tz + az,
            ],
            at,
          );
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
