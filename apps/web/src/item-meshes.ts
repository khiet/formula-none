// The items on the road (ADR 0037), built in code from Three primitives:
// lit, flat-shaded and in the palette the item slot draws them in, so the
// cone and the gull on the road are the ones in the slot. The runaway wheel
// and the marshal's flag (ADR 0041) are built the same way. The sim collides
// with its own sizes; these only fill them.

import {
  BoxGeometry,
  type BufferGeometry,
  ConeGeometry,
  CylinderGeometry,
  Group,
  Mesh,
  MeshLambertMaterial,
  SphereGeometry,
} from "three";
import {
  GREYS,
  GULL_EYE,
  HAZARD_ORANGE,
  INK,
  ITEM_BOX_GOLD,
  KART_RIM,
  KART_TYRE,
  SIGNAL_YELLOW,
  WHITE,
} from "./palette";

/** The item box: a cube this many metres across. */
const ITEM_BOX_SIZE_M = 1;
/** Each face's inset frame: its share of the face, and how far it stands proud. */
const ITEM_BOX_INSET = 0.7;
const ITEM_BOX_INSET_DEPTH_M = 0.02;

/** The hazard cone (ADR 0007), on a grey slab. */
const HAZARD_CONE_RADIUS_M = 0.5;
const HAZARD_CONE_HEIGHT_M = 0.6;
const HAZARD_SLAB_M = 1;
const HAZARD_SLAB_HEIGHT_M = 0.08;
/** The white band, as shares of the way from the apex to the rim. */
const HAZARD_BAND_FROM = 0.5;
const HAZARD_BAND_TO = 0.66;

/**
 * The runaway wheel: the kart's octagonal tyre and three-spoke rim, grown
 * to the sim's reach (`WHEEL_RADIUS_M` in tuning.rs), proportioned as the
 * kart's rear.
 */
export const RUNAWAY_WHEEL_RADIUS_M = 0.45;
const RUNAWAY_WHEEL_WIDTH_M = 0.36;
const RUNAWAY_WHEEL_SIDES = 8;
const RUNAWAY_WHEEL_RIM_SHARE = 0.58;
const RUNAWAY_WHEEL_SPOKES = 3;
/** How high the wheel's drawn bounces carry it, and how many it makes a pass. */
const RUNAWAY_WHEEL_BOUNCE_M = 0.5;
const RUNAWAY_WHEEL_BOUNCES = 3;

/** The marshal's flag: a pole, and the cloth flying from its top. */
const FLAG_POLE_M = 1.4;
const FLAG_POLE_THICKNESS_M = 0.05;
const FLAG_WIDTH_M = 0.9;
const FLAG_HEIGHT_M = 0.6;

function material(colour: number): MeshLambertMaterial {
  return new MeshLambertMaterial({ color: colour, flatShading: true });
}

function part(
  parent: Group,
  geometry: BufferGeometry,
  colour: number,
  [x, y, z]: readonly [number, number, number] = [0, 0, 0],
): Mesh {
  const mesh = new Mesh(geometry, material(colour));
  mesh.position.set(x, y, z);
  parent.add(mesh);
  return mesh;
}

/**
 * An item box, centred on its origin: a cube in the item box's lit gold,
 * so the sun's shading lands it on the slot's golds, with an inset frame
 * on each face in the mid gold: the box the item slot shows face on.
 */
export function itemBoxMesh(): Group {
  const box = new Group();
  part(box, new BoxGeometry(ITEM_BOX_SIZE_M, ITEM_BOX_SIZE_M, ITEM_BOX_SIZE_M), ITEM_BOX_GOLD.lit);
  const inset = ITEM_BOX_SIZE_M * ITEM_BOX_INSET;
  const panel = new BoxGeometry(inset, inset, ITEM_BOX_INSET_DEPTH_M);
  const out = ITEM_BOX_SIZE_M / 2;
  for (const [axis, sign] of [
    ["x", 1],
    ["x", -1],
    ["y", 1],
    ["y", -1],
    ["z", 1],
    ["z", -1],
  ] as const) {
    const face = part(box, panel, ITEM_BOX_GOLD.mid);
    face.position[axis] = sign * out;
    if (axis === "x") face.rotation.y = Math.PI / 2;
    if (axis === "y") face.rotation.x = Math.PI / 2;
  }
  return box;
}

/**
 * The hazard cone standing on its origin: six faceted sides in hazard
 * orange on a grey slab, a white band round it and, on the side facing
 * +z, the slot's grumpy face: eyes on the band, V brows over them and a
 * flat mouth under.
 */
export function hazardConeMesh(): Group {
  const cone = new Group();
  part(cone, new BoxGeometry(HAZARD_SLAB_M, HAZARD_SLAB_HEIGHT_M, HAZARD_SLAB_M), GREYS.dark, [
    0,
    HAZARD_SLAB_HEIGHT_M / 2,
    0,
  ]);
  // Turned a twelfth so a flat side, not an edge, faces +z.
  const turn = Math.PI / 6;
  const base = HAZARD_SLAB_HEIGHT_M;
  part(
    cone,
    new ConeGeometry(HAZARD_CONE_RADIUS_M, HAZARD_CONE_HEIGHT_M, 6, 1, false, turn),
    HAZARD_ORANGE.mid,
    [0, base + HAZARD_CONE_HEIGHT_M / 2, 0],
  );
  // At a share `t` of the way down from the apex: the height, and the side's radius.
  const heightAt = (t: number) => base + HAZARD_CONE_HEIGHT_M * (1 - t);
  const radiusAt = (t: number) => HAZARD_CONE_RADIUS_M * t;
  // Stood a hair off the cone so the band and the face never fight it for depth.
  const proud = 1.03;
  part(
    cone,
    new CylinderGeometry(
      radiusAt(HAZARD_BAND_FROM) * proud,
      radiusAt(HAZARD_BAND_TO) * proud,
      HAZARD_CONE_HEIGHT_M * (HAZARD_BAND_TO - HAZARD_BAND_FROM),
      6,
      1,
      true,
      turn,
    ),
    WHITE.lit,
    [0, (heightAt(HAZARD_BAND_FROM) + heightAt(HAZARD_BAND_TO)) / 2, 0],
  );

  // The front side leans back by the slope of its centre line.
  const apothem = Math.cos(Math.PI / 6);
  const lean = Math.atan((HAZARD_CONE_RADIUS_M * apothem) / HAZARD_CONE_HEIGHT_M);
  const feature = (t: number, x: number, width: number, height: number, tilt = 0) => {
    const piece = part(cone, new BoxGeometry(width, height, 0.01), INK, [
      x,
      heightAt(t),
      radiusAt(t) * apothem + 0.012,
    ]);
    piece.rotation.set(-lean, 0, tilt);
  };
  for (const side of [-1, 1]) {
    feature(0.53, side * 0.06, 0.035, 0.045);
    // The brows dip toward the nose.
    feature(0.43, side * 0.055, 0.06, 0.016, side * 0.45);
  }
  feature(0.73, 0, 0.08, 0.016);
  return cone;
}

/**
 * The homing projectile as a low-poly gull centred on its origin, beak
 * toward +z so it can be turned to face its target: white body and head,
 * grey wings up, a dark tail, an orange beak and red eyes locked on.
 */
export function homingGullMesh(): Group {
  const gull = new Group();
  const body = part(gull, new SphereGeometry(0.3, 6, 4), GREYS.white);
  body.scale.set(0.75, 0.65, 1.3);
  part(gull, new SphereGeometry(0.17, 6, 4), GREYS.white, [0, 0.14, 0.38]);
  const beak = part(gull, new ConeGeometry(0.06, 0.2, 4), HAZARD_ORANGE.mid, [0, 0.12, 0.6]);
  beak.rotation.x = Math.PI / 2;
  const tail = part(gull, new ConeGeometry(0.14, 0.3, 4), GREYS.dark, [0, 0.02, -0.45]);
  tail.rotation.x = -Math.PI / 2;
  for (const side of [-1, 1]) {
    const wing = part(gull, new BoxGeometry(0.5, 0.03, 0.28), GREYS.grey, [
      side * 0.32,
      0.26,
      -0.02,
    ]);
    wing.rotation.z = side * 0.9;
    part(gull, new BoxGeometry(0.02, 0.06, 0.06), GULL_EYE, [side * 0.15, 0.19, 0.44]);
    part(gull, new BoxGeometry(0.02, 0.03, 0.03), INK, [side * 0.16, 0.19, 0.46]);
  }
  return gull;
}

/**
 * The runaway wheel centred on its axle, which lies along x, so it rolls
 * toward -z: a dark tyre of eight facets and, proud of each face, a grey
 * rim with three spokes.
 */
export function runawayWheelMesh(): Group {
  const wheel = new Group();
  const radius = RUNAWAY_WHEEL_RADIUS_M;
  const tyre = part(
    wheel,
    new CylinderGeometry(radius, radius, RUNAWAY_WHEEL_WIDTH_M, RUNAWAY_WHEEL_SIDES),
    KART_TYRE,
  );
  tyre.rotation.z = Math.PI / 2;
  const rimRadius = radius * RUNAWAY_WHEEL_RIM_SHARE;
  for (const side of [-1, 1]) {
    const face = (side * RUNAWAY_WHEEL_WIDTH_M) / 2;
    const rim = part(
      wheel,
      new CylinderGeometry(rimRadius, rimRadius, 0.02, RUNAWAY_WHEEL_SIDES),
      KART_RIM,
      [face, 0, 0],
    );
    rim.rotation.z = Math.PI / 2;
    // Spokes reach from the rim out over the tyre's wall, so the turn reads.
    for (let spoke = 0; spoke < RUNAWAY_WHEEL_SPOKES; spoke += 1) {
      const turn = (2 * Math.PI * spoke) / RUNAWAY_WHEEL_SPOKES;
      const bar = part(wheel, new BoxGeometry(0.03, radius * 0.9, 0.05), KART_RIM, [
        face + side * 0.02,
        ((radius * 0.9) / 2) * Math.cos(turn),
        ((radius * 0.9) / 2) * Math.sin(turn),
      ]);
      bar.rotation.x = turn;
    }
  }
  return wheel;
}

/**
 * How high the drawn wheel bounces `fraction` of the way through a pass:
 * down at both walls and between each of its three bounces. Drawn only;
 * the sim rolls the wheel along the ground.
 */
export function runawayWheelBounce(fraction: number): number {
  return RUNAWAY_WHEEL_BOUNCE_M * Math.abs(Math.sin(fraction * Math.PI * RUNAWAY_WHEEL_BOUNCES));
}

/**
 * The marshal's yellow flag standing on its origin: a white pole and the
 * cloth flying from its top toward +x. The View waves it by turning it
 * about z.
 */
export function marshalFlagMesh(): Group {
  const flag = new Group();
  part(
    flag,
    new BoxGeometry(FLAG_POLE_THICKNESS_M, FLAG_POLE_M, FLAG_POLE_THICKNESS_M),
    WHITE.shaded,
    [0, FLAG_POLE_M / 2, 0],
  );
  part(flag, new BoxGeometry(FLAG_WIDTH_M, FLAG_HEIGHT_M, 0.02), SIGNAL_YELLOW.mid, [
    FLAG_WIDTH_M / 2,
    FLAG_POLE_M - FLAG_HEIGHT_M / 2,
    0,
  ]);
  return flag;
}
