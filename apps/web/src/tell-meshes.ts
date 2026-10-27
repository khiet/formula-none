// The Three.js side of the tells: one mesh per tell, sized once to its
// store's capacity and rewritten in place each frame, so eight karts' marks
// are one draw call and each particle pool another, and the scene graph
// never churns.
// Presentation only; everything drawn here was decided by a tell module
// from the transforms the bridge reports.

import {
  AdditiveBlending,
  BufferAttribute,
  BufferGeometry,
  CircleGeometry,
  Color,
  ConeGeometry,
  DoubleSide,
  Group,
  InstancedMesh,
  type Material,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  type Object3D,
  Quaternion,
  SphereGeometry,
  Vector3,
} from "three";
import { attribute, float, smoothstep, uv } from "three/tsl";
import { MeshBasicNodeMaterial } from "three/webgpu";
import type { KartSize } from "./bridge";
import type { ExhaustFlames } from "./exhaust-flame";
import { TYRE_WIDTH_M } from "./kart-frame";
import type { KartShadows } from "./kart-shadows";
import type { Particle, Particles } from "./particles";
import { TIER_COLOURS } from "./sparks";
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

export interface ParticleMeshOptions {
  /** Metres across an instance is drawn for a particle of `size` with `life` left. */
  scaleOf(size: number, life: number): number;
  /** Draw each particle as a streak from its tail to it, as thick as its scale; a point when omitted. */
  readonly streak?: boolean;
  /** Only the particles this passes are drawn; every one when omitted. */
  include?(particle: Particle): boolean;
  /**
   * How bright, 0 to 1, a particle's colour is drawn; full when omitted. In
   * an additive material dimmer is more see-through, so this fades it.
   */
  brightnessOf?(particle: Particle): number;
}

/** A material that adds its colour to what is behind it, so black draws nothing and overlaps glow. */
export function additiveMaterial(): MeshBasicMaterial {
  return new MeshBasicMaterial({
    blending: AdditiveBlending,
    transparent: true,
    depthWrite: false,
  });
}

const ALONG_Z = new Vector3(0, 0, 1);

/**
 * Every live particle `options` includes as one instance of `geometry` in
 * `material`, placed at the particle, or stretched back to its tail when
 * `options.streak`, scaled to `options.scaleOf` its size and life, in its
 * colour. Dead slots are not drawn: the instance count is the live count.
 */
export function particleMesh(
  particles: Particles,
  geometry: BufferGeometry,
  material: Material,
  options: ParticleMeshOptions,
): TellMesh {
  const mesh = new InstancedMesh(geometry, material, particles.capacity);
  mesh.frustumCulled = false;
  // Colour every instance once so the colour buffer exists to rewrite.
  const colour = new Color();
  for (let instance = 0; instance < particles.capacity; instance++) {
    mesh.setColorAt(instance, colour);
  }
  const placement = new Matrix4();
  const at = new Vector3();
  const along = new Vector3();
  const turn = new Quaternion();
  const scale = new Vector3();
  return {
    mesh,
    update() {
      let live = 0;
      particles.forEach((particle) => {
        if (options.include !== undefined && !options.include(particle)) {
          return;
        }
        const thickness = options.scaleOf(particle.size, particle.life);
        const [x, y, z] = particle.position;
        const [tx, ty, tz] = particle.tail;
        along.set(x - tx, y - ty, z - tz);
        const length = options.streak === true ? along.length() : 0;
        if (length > 1e-6) {
          // Centred between tail and head, its local Z along the streak.
          at.set((x + tx) / 2, (y + ty) / 2, (z + tz) / 2);
          turn.setFromUnitVectors(ALONG_Z, along.divideScalar(length));
          scale.set(thickness, thickness, thickness + length);
        } else {
          at.set(x, y, z);
          turn.identity();
          scale.setScalar(thickness);
        }
        mesh.setMatrixAt(live, placement.compose(at, turn, scale));
        colour.setHex(particle.colour);
        if (options.brightnessOf !== undefined) {
          colour.multiplyScalar(options.brightnessOf(particle));
        }
        mesh.setColorAt(live, colour);
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

/** A flame at full strength is this wide, its core and halo a share of it. */
const FLAME_RADIUS_M = 0.32;
const FLAME_CORE_SHARE = 0.3;
const FLAME_HALO_SHARE = 1.6;
const FLAME_HALO_BRIGHTNESS = 0.6;
const FLAME_TONGUE_BRIGHTNESS = 0.75;
/** Tongues per flame, splayed this far off straight back, rocking round at this rate. */
const FLAME_TONGUES = 6;
const FLAME_SPLAY_RAD = 0.35;
const FLAME_ROCK_RAD_PER_S = 3;
/** A tongue reaches between these multiples of the radius, flickering at this rate. */
const FLAME_REACH_LEAST = 1.6;
const FLAME_REACH_FLICKER = 1.4;
const FLAME_FLICKER_RAD_PER_S = 40;
const FLAME_TONGUE_BASE_SHARE = 0.35;
/** The flame sits just behind the tail so the kart's own faces do not cut it. */
const FLAME_LIFT_M = 0.05;

/**
 * Every burning exhaust flame as a hot white core, a halo in its tier's
 * birth colour and flickering tongues in the tier's cooled colour, all
 * additive and scaled by the flame's strength: three instanced meshes for
 * the grid. Advance with the frame's seconds, so the flicker runs at the
 * same rate whatever the frame rate.
 */
export function exhaustFlameMesh(
  flames: ExhaustFlames,
): Omit<TellMesh, "update"> & { update(frameSeconds: number): void } {
  const count = flames.flames.length;
  const material = additiveMaterial();
  const cores = new InstancedMesh(new SphereGeometry(1, 8, 6), material, count);
  const halos = new InstancedMesh(new SphereGeometry(1, 8, 6), material, count);
  // A unit cone with its base at the origin and its tip at +Y.
  const tongueGeometry = new ConeGeometry(1, 1, 5).translate(0, 0.5, 0);
  const tongues = new InstancedMesh(tongueGeometry, material, count * FLAME_TONGUES);
  const group = new Group();
  const colour = new Color();
  for (const mesh of [cores, halos, tongues]) {
    mesh.frustumCulled = false;
    for (let instance = 0; instance < mesh.count; instance++) {
      mesh.setColorAt(instance, colour);
    }
    group.add(mesh);
  }
  const placement = new Matrix4();
  const at = new Vector3();
  const back = new Vector3();
  const right = new Vector3();
  const up = new Vector3();
  const reach = new Vector3();
  const turn = new Quaternion();
  const scale = new Vector3();
  const worldUp = new Vector3(0, 1, 0);
  const alongY = new Vector3(0, 1, 0);
  let seconds = 0;
  return {
    mesh: group,
    update(frameSeconds) {
      seconds += frameSeconds;
      let live = 0;
      for (const flame of flames.flames) {
        if (!flame.visible || flame.strength <= 0) {
          continue;
        }
        const { hot, cooled } = TIER_COLOURS[flame.tier];
        const radius = FLAME_RADIUS_M * flame.strength;
        back.set(...flame.back);
        at.set(...flame.position).addScaledVector(back, FLAME_LIFT_M);
        turn.identity();
        cores.setMatrixAt(
          live,
          placement.compose(at, turn, scale.setScalar(radius * FLAME_CORE_SHARE)),
        );
        cores.setColorAt(live, colour.setHex(0xffffff).multiplyScalar(flame.strength));
        halos.setMatrixAt(
          live,
          placement.compose(at, turn, scale.setScalar(radius * FLAME_HALO_SHARE)),
        );
        halos.setColorAt(
          live,
          colour.setHex(hot).multiplyScalar(FLAME_HALO_BRIGHTNESS * flame.strength),
        );
        right.crossVectors(back, worldUp);
        if (right.lengthSq() < 1e-6) {
          right.set(1, 0, 0);
        }
        right.normalize();
        up.crossVectors(right, back);
        colour.setHex(cooled).multiplyScalar(FLAME_TONGUE_BRIGHTNESS * flame.strength);
        for (let tongue = 0; tongue < FLAME_TONGUES; tongue++) {
          const around = (tongue / FLAME_TONGUES) * 2 * Math.PI + seconds * FLAME_ROCK_RAD_PER_S;
          const flicker = Math.abs(Math.sin(seconds * FLAME_FLICKER_RAD_PER_S + tongue * 2.3));
          reach
            .copy(back)
            .multiplyScalar(Math.cos(FLAME_SPLAY_RAD))
            .addScaledVector(right, Math.cos(around) * Math.sin(FLAME_SPLAY_RAD))
            .addScaledVector(up, Math.sin(around) * Math.sin(FLAME_SPLAY_RAD));
          turn.setFromUnitVectors(alongY, reach);
          const base = radius * FLAME_TONGUE_BASE_SHARE;
          scale.set(base, radius * (FLAME_REACH_LEAST + FLAME_REACH_FLICKER * flicker), base);
          const instance = live * FLAME_TONGUES + tongue;
          tongues.setMatrixAt(instance, placement.compose(at, turn, scale));
          tongues.setColorAt(instance, colour);
        }
        live += 1;
      }
      cores.count = live;
      halos.count = live;
      tongues.count = live * FLAME_TONGUES;
      for (const mesh of [cores, halos, tongues]) {
        mesh.instanceMatrix.needsUpdate = true;
        if (mesh.instanceColor !== null) {
          mesh.instanceColor.needsUpdate = true;
        }
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
