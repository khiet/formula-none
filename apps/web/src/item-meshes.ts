// The items on the road (ADR 0037), built in code from Three primitives:
// lit, flat-shaded and in the palette the item slot draws them in, so the
// cone and the gull on the road are the ones in the slot. The runaway wheel
// and the marshal's flag (ADR 0041) are built the same way, save the
// wheel's rim, hub and flames, which glow unlit. The sim collides with its own
// sizes; these only fill them.

import {
  BoxGeometry,
  type BufferGeometry,
  CircleGeometry,
  Color,
  ConeGeometry,
  CylinderGeometry,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  SphereGeometry,
} from "three";
import {
  GREYS,
  GULL_EYE,
  HAZARD_ORANGE,
  INK,
  ITEM_BOX_GOLD,
  ROCKET,
  ROCKET_FLAME,
  ROCKET_STRAP,
  SIGNAL_YELLOW,
  WHEEL_FLAME,
  WHEEL_TYRE,
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

/** The runaway wheel's width over its radius: the kart's rear wheel's. */
const RUNAWAY_WHEEL_WIDTH_SHARE = 0.8;
const RUNAWAY_WHEEL_SIDES = 8;
const RUNAWAY_WHEEL_RIM_SHARE = 0.58;
const RUNAWAY_WHEEL_HUB_SHARE = 0.14;
const RUNAWAY_WHEEL_SPOKES = 3;
/**
 * The flames off its back, as shares of its radius: how far each tongue
 * reaches and how wide its base is, where it rises from, and its core's
 * share of it.
 */
const WHEEL_FLAME_LENGTH_SHARE = 1;
const WHEEL_FLAME_RADIUS_SHARE = 0.22;
const WHEEL_FLAME_CORE_SHARE = 0.55;
const WHEEL_FLAME_OPACITY = 0.75;
/** Each tongue's base, [across the tyre's width, up], as shares of the width and the radius. */
const WHEEL_FLAME_TONGUES: readonly (readonly [number, number])[] = [
  [-0.3, 0.35],
  [0, 0.6],
  [0.3, 0.35],
];
/** How high the wheel's drawn bounces carry it, and how many it makes a leg. */
const RUNAWAY_WHEEL_BOUNCE_M = 0.5;
const RUNAWAY_WHEEL_BOUNCES_PER_LEG = 2;

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

/** A mesh in `colour` that ignores the sun, so it glows; at `opacity` under 1, what is inside shows through. */
function glowing(parent: Group, geometry: BufferGeometry, colour: number, opacity = 1): Mesh {
  const material = new MeshBasicMaterial({ color: colour });
  if (opacity < 1) {
    material.transparent = true;
    material.opacity = opacity;
    material.depthWrite = false;
  }
  const mesh = new Mesh(geometry, material);
  parent.add(mesh);
  return mesh;
}

/**
 * The runaway wheel centred on its axle, which lies along x, so it rolls
 * toward -z: the hot wheel, an octagonal tyre dark against the asphalt
 * with a red-hot rim on each face, flame yellow at the hub cooling to the
 * rocket's red at its edge and unlit so it glows, three spokes in the
 * rocket's strap and a flame-yellow hub. Grown to `radius`, which the
 * caller takes from the sim's reach so the wheel drawn is the wheel that
 * hits.
 */
export function runawayWheelMesh(radius: number): Group {
  const wheel = new Group();
  const width = radius * RUNAWAY_WHEEL_WIDTH_SHARE;
  const tyre = part(
    wheel,
    new CylinderGeometry(radius, radius, width, RUNAWAY_WHEEL_SIDES),
    WHEEL_TYRE,
  );
  tyre.rotation.z = Math.PI / 2;
  const rim = hotRimGeometry(radius * RUNAWAY_WHEEL_RIM_SHARE);
  const rimMaterial = new MeshBasicMaterial({ vertexColors: true });
  const hubRadius = radius * RUNAWAY_WHEEL_HUB_SHARE;
  for (const side of [-1, 1]) {
    // A hair proud of the tyre's face so the two never fight for depth.
    const face = (side * width) / 2 + side * 0.01;
    const disc = new Mesh(rim, rimMaterial);
    disc.position.x = face;
    // The disc faces +z; turned a quarter it faces out of this side.
    disc.rotation.y = (side * Math.PI) / 2;
    wheel.add(disc);
    // Spokes reach from the hub out over the tyre's wall, so the turn reads,
    // standing proud of the rim, and the hub proud of the spokes.
    for (let spoke = 0; spoke < RUNAWAY_WHEEL_SPOKES; spoke += 1) {
      const turn = (2 * Math.PI * spoke) / RUNAWAY_WHEEL_SPOKES;
      const bar = part(wheel, new BoxGeometry(0.03, radius * 0.9, 0.05), ROCKET_STRAP, [
        face + side * 0.02,
        ((radius * 0.9) / 2) * Math.cos(turn),
        ((radius * 0.9) / 2) * Math.sin(turn),
      ]);
      bar.rotation.x = turn;
    }
    const hub = glowing(
      wheel,
      new CylinderGeometry(hubRadius, hubRadius, 0.06, RUNAWAY_WHEEL_SIDES),
      ROCKET_FLAME,
    );
    hub.position.x = face + side * 0.04;
    hub.rotation.z = Math.PI / 2;
  }
  return wheel;
}

/** An octagonal disc `radius` across, facing +z: flame yellow at its centre, the rocket's red at its edge. */
function hotRimGeometry(radius: number): BufferGeometry {
  const disc = new CircleGeometry(radius, RUNAWAY_WHEEL_SIDES);
  // The circle's first vertex is its centre; the rest ring its edge.
  const centre = new Color(ROCKET_FLAME);
  const edge = new Color(ROCKET.mid);
  const count = disc.getAttribute("position").count;
  const colours: number[] = [];
  for (let vertex = 0; vertex < count; vertex++) {
    const colour = vertex === 0 ? centre : edge;
    colours.push(colour.r, colour.g, colour.b);
  }
  disc.setAttribute("color", new Float32BufferAttribute(colours, 3));
  return disc;
}

/**
 * The runaway wheel's flames for a wheel `radius` in size, centred on its
 * axle and pointing toward +z, which the View turns to the wheel's back:
 * three tongues rising off the back of the tyre, the flames' orange round
 * a flame-yellow core, unlit and the orange see-through. Each tongue is a child whose length runs
 * along its z, so the View flickers it by scaling z.
 */
export function runawayWheelFlames(radius: number): Group {
  const flames = new Group();
  const length = radius * WHEEL_FLAME_LENGTH_SHARE;
  const base = radius * WHEEL_FLAME_RADIUS_SHARE;
  // A cone along +z from its base at the origin to its tip `reach` out.
  const tongue = (width: number, reach: number) =>
    new ConeGeometry(width, reach, 4).translate(0, reach / 2, 0).rotateX(Math.PI / 2);
  const outer = tongue(base, length);
  const inner = tongue(base * WHEEL_FLAME_CORE_SHARE, length * WHEEL_FLAME_CORE_SHARE);
  const width = radius * RUNAWAY_WHEEL_WIDTH_SHARE;
  for (const [across, up] of WHEEL_FLAME_TONGUES) {
    const group = new Group();
    // Rooted a fifth of the way in from the back of the tread, so no gap shows between flame and tyre.
    const height = up * radius;
    group.position.set(across * width, height, Math.sqrt(radius * radius - height * height) * 0.8);
    // See-through, so the core burns yellow inside it.
    glowing(group, outer, WHEEL_FLAME, WHEEL_FLAME_OPACITY);
    glowing(group, inner, ROCKET_FLAME);
    flames.add(group);
  }
  return flames;
}

/**
 * How high the drawn wheel bounces `legFraction` of the way through a leg
 * of its pass, wall to wall: down at each wall and between its two
 * bounces, so they start over at each wall it rebounds from. Drawn only;
 * the sim rolls the wheel along the ground.
 */
export function runawayWheelBounce(legFraction: number): number {
  return (
    RUNAWAY_WHEEL_BOUNCE_M *
    Math.abs(Math.sin(legFraction * Math.PI * RUNAWAY_WHEEL_BOUNCES_PER_LEG))
  );
}

/**
 * Whether the drawn wheel comes down from a bounce between `from` and `to`
 * of the way through one leg, short of the wall that ends it.
 */
export function runawayWheelLanded(from: number, to: number): boolean {
  return (
    to < 1 &&
    Math.floor(to * RUNAWAY_WHEEL_BOUNCES_PER_LEG) >
      Math.floor(from * RUNAWAY_WHEEL_BOUNCES_PER_LEG)
  );
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
