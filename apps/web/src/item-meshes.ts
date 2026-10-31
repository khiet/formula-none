// The items on the road (ADR 0037), built in code from Three primitives:
// lit, flat-shaded and in the palette the item slot draws them in, so the
// cone and the drone on the road are the ones in the slot. The runaway wheel
// (ADR 0041) and the marshal waving his hazard flags at it (ADR 0045) are
// built the same way, save the wheel's rim, hub and flames and the
// beacon's glow and beam, which glow unlit. The sim collides with its own
// sizes; these only fill them.

import {
  AdditiveBlending,
  BoxGeometry,
  BufferGeometry,
  CircleGeometry,
  Color,
  ConeGeometry,
  CylinderGeometry,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  SphereGeometry,
  TorusGeometry,
  Vector3,
} from "three";
import type { ItemBox, Wheel } from "./bridge";
import {
  BEACON_BEAM,
  BEACON_LIT,
  BEACON_OFF,
  CARBON,
  DRONE_EYE,
  GREYS,
  HAZARD_FLAG_STRIPE,
  HAZARD_ORANGE,
  INK,
  ITEM_BOX_GOLD,
  MARSHAL_BROWS,
  MARSHAL_EYE,
  MARSHAL_HAIR,
  MARSHAL_JACKET,
  MARSHAL_MOUTH,
  MARSHAL_MUSTACHE,
  MARSHAL_NOSE,
  MARSHAL_REFLECTIVE,
  MARSHAL_SANDALS,
  MARSHAL_SHORTS,
  MARSHAL_SKIN,
  MARSHAL_STOOL,
  MARSHAL_SWEAT,
  MARSHAL_TOWEL,
  MARSHAL_VEST,
  POSTER_RED,
  ROCKET,
  ROCKET_FLAME,
  ROCKET_STRAP,
  SIGNAL_YELLOW,
  VALUE_WHITE,
  WHEEL_FLAME,
  WHEEL_TYRE,
  WHITE,
} from "./palette";

/** The item box: a cube this many metres across. */
const ITEM_BOX_SIZE_M = 1;
/** Each face's inset frame: its share of the face, and how far it stands proud. */
const ITEM_BOX_INSET = 0.7;
const ITEM_BOX_INSET_DEPTH_M = 0.02;
/** The box's centre over the asphalt, before the bob. */
const ITEM_BOX_HEIGHT_M = 1;
/** One turn about the vertical, half the coin's pace, so the bigger thing reads heavier. */
const ITEM_BOX_TURN_S = 2;
/** The coin's bob, so the boxes and the coins rise and fall in step. */
const ITEM_BOX_BOB_M = 0.05;
const ITEM_BOX_BOB_S = 2;

/** The hazard cone (ADR 0007), on a grey slab. */
const HAZARD_CONE_RADIUS_M = 0.5;
const HAZARD_CONE_HEIGHT_M = 0.6;
const HAZARD_SLAB_M = 1;
const HAZARD_SLAB_HEIGHT_M = 0.08;
/** The white band, as shares of the way from the apex to the rim. */
const HAZARD_BAND_FROM = 0.5;
const HAZARD_BAND_TO = 0.66;

/**
 * The homing drone: its motors this far out on each axis, its rotors this
 * big round and their blur this opaque. Its rotors spin this fast, its
 * tally blinks this often, and it bobs this high this fast as it hovers.
 */
const DRONE_MOTOR_M = 0.32;
const DRONE_ROTOR_M = 0.19;
const DRONE_BLUR_OPACITY = 0.22;
const DRONE_ROTOR_RAD_PER_S = 38;
const DRONE_TALLY_HZ = 1.5;
const DRONE_BOB_M = 0.02;
const DRONE_BOB_RAD_PER_S = 3.1;

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

/** A hazard flag: a pole, and the cloth flying from its top, striped on the diagonal. */
const FLAG_POLE_M = 1;
const FLAG_POLE_THICKNESS_M = 0.04;
const FLAG_WIDTH_M = 0.9;
const FLAG_HEIGHT_M = 0.62;
const FLAG_STRIPE_M = 0.09;
const FLAG_STRIPE_EVERY_M = 0.2;
/**
 * The cloth ripples out from the pole this fast, turning this far across
 * its width, its free edge swinging this share of its height.
 */
const FLAG_RIPPLE_RAD_PER_S = 12;
const FLAG_RIPPLE_RAD_ACROSS = 6.4;
const FLAG_RIPPLE_SHARE = 0.13;
/** Furled, the cloth is a roll this thick down the pole, banded black this many times. */
const FLAG_FURL_M = 0.08;
const FLAG_FURL_BANDS = 4;

/**
 * The marshal, chibi: a faceted head this big round, its centre this high,
 * on a round belly, and his top about 1.75 m up. Seated on his stool he is
 * this much lower; while the wheel rolls he hops this high this fast.
 */
const MARSHAL_HEAD_RADIUS_M = 0.34;
const MARSHAL_HEAD_Y_M = 1.42;
const MARSHAL_SEATED_DROP_M = 0.15;
const MARSHAL_HOP_M = 0.18;
const MARSHAL_HOP_RAD_PER_S = 9;
/** His right shoulder, and his right hand held up with a flag; the left mirrors them. */
const MARSHAL_SHOULDER: readonly [number, number] = [0.29, 1];
const MARSHAL_HAND_UP: readonly [number, number] = [0.55, 1.36];
/** He waves each flag this fast through this half-swing about this tilt out from his head. */
const FLAG_WAVE_RAD_PER_S = 14;
const FLAG_WAVE_SWING_RAD = (34 * Math.PI) / 180;
const FLAG_WAVE_OUT_RAD = (18 * Math.PI) / 180;
/** In a panic his mouth chatters this fast and sweat flies off him this often a second. */
const MARSHAL_CHATTER_RAD_PER_S = 25;
const MARSHAL_SWEAT_PER_S = 1.6;
/** Steam rises this often a second, this high, off his tea. */
const MARSHAL_STEAM_PER_S = 0.6;
const MARSHAL_STEAM_RISE_M = 0.3;

/**
 * The marshal post's beacon: its size, and its beam's reach, opacity, and
 * sweep side to side this fast through this half-swing.
 */
const BEACON_RADIUS_M = 0.13;
const BEACON_BEAM_M = 1.2;
const BEACON_BEAM_RAD_PER_S = 11;
const BEACON_BEAM_SWING_RAD = 0.8;
const BEACON_BEAM_OPACITY = 0.4;
const BEACON_GLOW_OPACITY = 0.35;

function material(colour: number): MeshLambertMaterial {
  return new MeshLambertMaterial({ color: colour, flatShading: true });
}

function part(
  parent: Group,
  geometry: BufferGeometry,
  colour: number,
  [x, y, z]: readonly [number, number, number] = [0, 0, 0],
): Mesh<BufferGeometry, MeshLambertMaterial> {
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

/** The track's item boxes as the View draws them. */
export interface ItemBoxMeshes {
  readonly mesh: Group;
  /** Show each box, in the sidecar's order, while it is there to pick up. */
  show(present: readonly boolean[]): void;
  /** Turn and bob every box to `clockSeconds` on the View's clock. */
  turn(clockSeconds: number): void;
}

/**
 * A box at each of `placed`, sharing one box's geometries and materials:
 * each placed group holds its box, which turns and bobs inside it off the
 * View's clock, every box facing the same way at the same moment.
 */
export function itemBoxMeshes(placed: readonly ItemBox[]): ItemBoxMeshes {
  const mesh = new Group();
  const box = itemBoxMesh();
  const boxes = placed.map(({ position, yaw }) => {
    const group = new Group();
    group.position.set(...position);
    group.rotation.y = yaw;
    const clone = box.clone();
    clone.position.y = ITEM_BOX_HEIGHT_M;
    group.add(clone);
    mesh.add(group);
    return { group, clone, yaw };
  });
  return {
    mesh,
    show(present) {
      boxes.forEach(({ group }, index) => {
        group.visible = present[index] ?? true;
      });
    },
    turn(clockSeconds) {
      const angle = ((clockSeconds % ITEM_BOX_TURN_S) / ITEM_BOX_TURN_S) * 2 * Math.PI;
      const height =
        ITEM_BOX_HEIGHT_M +
        ITEM_BOX_BOB_M * Math.sin((clockSeconds / ITEM_BOX_BOB_S) * 2 * Math.PI);
      for (const { clone, yaw } of boxes) {
        // Less the placement's yaw, so every box faces the same way.
        clone.rotation.y = angle - yaw;
        clone.position.y = height;
      }
    },
  };
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

/** The homing drone at a moment: posed by `fly`. */
export interface HomingDrone {
  /** Centred on its origin, nose toward +z, so it can be turned to face its target. */
  readonly figure: Group;
  /**
   * Roll the whole drone `tilt` radians about its nose, positive dropping
   * its left side (+x), and spin its rotors, blink its tally and bob it
   * `clockSeconds` into the View's clock.
   */
  fly(tilt: number, clockSeconds: number): void;
}

/**
 * The homing projectile as a camera quad (ADR 0057), a trackside broadcast
 * FPV drone gone rogue: a carbon body under a red canopy, X arms out to
 * orange motors, a two-blade rotor over a translucent blur disc on each,
 * a camera pod under the nose whose red lens is its eye locked on, and an
 * antenna at the rear left with a tally light on its tip. The lens, the
 * blur and the tally glow unlit. Its rolling part is named `body`, its
 * rotors `rotor`, its tally `tally`.
 */
export function homingDroneMesh(): HomingDrone {
  const figure = new Group();
  const body = new Group();
  body.name = "body";
  figure.add(body);
  part(body, new BoxGeometry(0.3, 0.1, 0.42), CARBON.mid);
  const canopy = part(
    body,
    new SphereGeometry(0.15, 6, 3, 0, 2 * Math.PI, 0, Math.PI / 2),
    POSTER_RED.mid,
    [0, 0.05, -0.02],
  );
  canopy.scale.set(0.95, 0.8, 1.35);
  const rotors: { readonly blades: Group; readonly direction: number }[] = [];
  const blur = new CircleGeometry(DRONE_ROTOR_M, 10);
  for (const sx of [-1, 1]) {
    for (const sz of [-1, 1]) {
      const arm = part(body, new BoxGeometry(0.05, 0.035, 0.44), CARBON.lit, [
        sx * 0.16,
        0,
        sz * 0.16,
      ]);
      arm.rotation.y = Math.atan2(sx, sz);
      part(body, new CylinderGeometry(0.05, 0.055, 0.09, 6), HAZARD_ORANGE.mid, [
        sx * DRONE_MOTOR_M,
        0.03,
        sz * DRONE_MOTOR_M,
      ]);
      const blades = new Group();
      blades.name = "rotor";
      blades.position.set(sx * DRONE_MOTOR_M, 0.09, sz * DRONE_MOTOR_M);
      part(blades, new BoxGeometry(DRONE_ROTOR_M * 2, 0.012, 0.045), CARBON.mid);
      body.add(blades);
      const disc = glowing(body, blur, VALUE_WHITE, DRONE_BLUR_OPACITY);
      disc.material.side = DoubleSide;
      disc.position.copy(blades.position);
      disc.rotation.x = -Math.PI / 2;
      // Diagonal pairs spin opposite ways, as a quad's props do.
      rotors.push({ blades, direction: sx * sz });
    }
  }
  part(body, new BoxGeometry(0.15, 0.13, 0.12), CARBON.shaded, [0, -0.08, 0.22]);
  const ring = part(body, new CylinderGeometry(0.06, 0.06, 0.03, 8), WHITE.mid, [0, -0.08, 0.29]);
  ring.rotation.x = Math.PI / 2;
  glowing(body, new CircleGeometry(0.045, 8), DRONE_EYE).position.set(0, -0.08, 0.307);
  glowing(body, new CircleGeometry(0.015, 6), INK).position.set(0.005, -0.075, 0.309);
  const antenna = part(
    body,
    new CylinderGeometry(0.008, 0.008, 0.22, 4),
    ROCKET_STRAP,
    [0.08, 0.13, -0.22],
  );
  antenna.rotation.x = -0.5;
  const tally = glowing(body, new SphereGeometry(0.025, 6, 4), ROCKET.lit);
  tally.name = "tally";
  tally.position.set(0.08, 0.23, -0.27);
  const drone: HomingDrone = {
    figure,
    fly(tilt, clockSeconds) {
      body.rotation.z = -tilt;
      body.position.y = DRONE_BOB_M * Math.sin(clockSeconds * DRONE_BOB_RAD_PER_S);
      for (const { blades, direction } of rotors) {
        blades.rotation.y = direction * clockSeconds * DRONE_ROTOR_RAD_PER_S;
      }
      tally.visible = Math.floor(clockSeconds * DRONE_TALLY_HZ * 2) % 2 === 0;
    },
  };
  drone.fly(0, 0);
  return drone;
}

/** A mesh in `colour` that ignores the sun, so it glows; at `opacity` under 1, what is inside shows through. */
function glowing(
  parent: Group,
  geometry: BufferGeometry,
  colour: number,
  opacity = 1,
): Mesh<BufferGeometry, MeshBasicMaterial> {
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
 * toward -z: an octagonal tyre dark against the asphalt
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
 * a flame-yellow core, unlit and the orange see-through. Each tongue is a
 * child whose length runs along its z, so the View flickers it by scaling z.
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

/** A hazard flag the marshal waves. */
export interface HazardFlag {
  /** Standing on its origin, the pole up +y and the cloth flying from its top toward +x. */
  readonly mesh: Group;
  /** Fly the cloth, rippled as it is `clockSeconds` into the View's clock. */
  fly(clockSeconds: number): void;
  /** Furl the cloth down the pole. */
  furl(): void;
}

/**
 * A hazard flag: a 1 m pole in the white's shade and the cloth flying from
 * its top, signal yellow striped black on the diagonal. Not a racing flag,
 * so it is never read as one. The cloth is named `cloth`.
 */
export function hazardFlag(): HazardFlag {
  const mesh = new Group();
  part(
    mesh,
    new BoxGeometry(FLAG_POLE_THICKNESS_M, FLAG_POLE_M, FLAG_POLE_THICKNESS_M),
    WHITE.shaded,
    [0, FLAG_POLE_M / 2, 0],
  );
  const cloth = new Group();
  cloth.name = "cloth";
  const bands = clothBands();
  bands.forEach((geometry, index) => {
    const band = part(cloth, geometry, index === 0 ? SIGNAL_YELLOW.mid : HAZARD_FLAG_STRIPE);
    band.material.side = DoubleSide;
  });
  // Rolled down the pole beside it, banded where the stripes fold in.
  const furled = new Group();
  const rollX = (FLAG_POLE_THICKNESS_M + FLAG_FURL_M) / 2;
  part(furled, new BoxGeometry(FLAG_FURL_M, FLAG_HEIGHT_M, FLAG_FURL_M), SIGNAL_YELLOW.mid, [
    rollX,
    FLAG_POLE_M - FLAG_HEIGHT_M / 2,
    0,
  ]);
  const bandEvery = FLAG_HEIGHT_M / FLAG_FURL_BANDS;
  for (let band = 0; band < FLAG_FURL_BANDS; band++) {
    part(
      furled,
      new BoxGeometry(FLAG_FURL_M * 1.1, bandEvery / 3, FLAG_FURL_M * 1.1),
      HAZARD_FLAG_STRIPE,
      [rollX, FLAG_POLE_M - (band + 0.5) * bandEvery, 0],
    );
  }
  mesh.add(cloth, furled);
  const flag: HazardFlag = {
    mesh,
    fly(clockSeconds) {
      cloth.visible = true;
      furled.visible = false;
      for (const geometry of bands) {
        const positions = geometry.getAttribute("position");
        for (let vertex = 0; vertex < positions.count; vertex++) {
          // Pinned at the pole, swinging more the further out.
          const out = positions.getX(vertex) / FLAG_WIDTH_M;
          positions.setZ(
            vertex,
            FLAG_HEIGHT_M *
              FLAG_RIPPLE_SHARE *
              out *
              Math.sin(clockSeconds * FLAG_RIPPLE_RAD_PER_S - out * FLAG_RIPPLE_RAD_ACROSS),
          );
        }
        positions.needsUpdate = true;
        geometry.computeBoundingBox();
        geometry.computeBoundingSphere();
      }
    },
    furl() {
      cloth.visible = false;
      furled.visible = true;
    },
  };
  flag.fly(0);
  return flag;
}

type Point = readonly [number, number];

/**
 * The cloth flat in its plane, x out from the pole and y up, as two
 * geometries: the yellow, and the black stripes between it. Each band runs
 * between two lines of constant x + y, so it leans down away from the pole.
 */
function clothBands(): [BufferGeometry, BufferGeometry] {
  const foot = FLAG_POLE_M - FLAG_HEIGHT_M;
  const cloth: Point[] = [
    [0, foot],
    [FLAG_WIDTH_M, foot],
    [FLAG_WIDTH_M, FLAG_POLE_M],
    [0, FLAG_POLE_M],
  ];
  const yellow: number[] = [];
  const black: number[] = [];
  const band = (from: number, to: number, into: number[]) => {
    const clipped = clip(
      clip(cloth, ([x, y]) => x + y - from),
      ([x, y]) => to - x - y,
    );
    // Each band is convex, so a fan from its first corner covers it.
    const [first] = clipped;
    if (first === undefined) return;
    clipped.slice(1, -1).forEach((corner, index) => {
      const next = clipped[index + 2] ?? corner;
      for (const [x, y] of [first, corner, next]) into.push(x, y, 0);
    });
  };
  const first = Math.floor(foot / FLAG_STRIPE_EVERY_M) * FLAG_STRIPE_EVERY_M;
  for (let from = first; from < FLAG_WIDTH_M + FLAG_POLE_M; from += FLAG_STRIPE_EVERY_M) {
    band(from, from + FLAG_STRIPE_M, black);
    band(from + FLAG_STRIPE_M, from + FLAG_STRIPE_EVERY_M, yellow);
  }
  return [yellow, black].map((positions) => {
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
    geometry.computeVertexNormals();
    return geometry;
  }) as [BufferGeometry, BufferGeometry];
}

/** What of convex `polygon` lies where `inside` is not negative; `inside` is linear. */
function clip(polygon: readonly Point[], inside: (point: Point) => number): Point[] {
  const kept: Point[] = [];
  polygon.forEach((point, index) => {
    const next = polygon[(index + 1) % polygon.length] ?? point;
    const here = inside(point);
    const there = inside(next);
    if (here >= 0) kept.push(point);
    if (here >= 0 !== there >= 0) {
      const share = here / (here - there);
      kept.push([point[0] + (next[0] - point[0]) * share, point[1] + (next[1] - point[1]) * share]);
    }
  });
  return kept;
}

/** The marshal at a marshal post. */
export interface Marshal {
  /** Standing on his origin, facing +z. */
  readonly figure: Group;
  /** Pose him for the wheel's `kind`, `clockSeconds` into the View's clock. */
  pose(kind: Wheel["kind"], clockSeconds: number): void;
}

const DOWN = new Vector3(0, -1, 0);

/**
 * The marshal, flat-shaded from primitives in Purrjump's colours for him:
 * bald with a shine, grey side hair, heavy brows and a wide mustache on a
 * faceted head, a round belly in a hi-vis jacket with a reflective band
 * over a white vest, a towel round his neck, shorts and sandals, a hazard
 * flag in each hand. Quiet, he sits on a green folding stool sipping tea,
 * steam rising, calm, his flags furled against the stool. From the flag
 * going up he is on his feet in a panic, arms up waving the flags out of
 * phase, and while the wheel rolls he hops. His parts are named for the
 * tests: `head`, `stool`, `cup`, `face.calm`, `face.panic`, and
 * `flag.left` and `flag.right`, left and right as seen from in front.
 */
export function marshalMesh(): Marshal {
  const figure = new Group();
  // Everything above his legs, which sits and hops with him.
  const body = new Group();
  const standingLegs = new Group();
  // Knees apart, thighs along the stool and shins down to the ground.
  const seatedLegs = new Group();
  figure.add(body, standingLegs, seatedLegs);
  for (const side of [-1, 1]) {
    part(standingLegs, new BoxGeometry(0.13, 0.33, 0.13), MARSHAL_SKIN.mid, [
      side * 0.145,
      0.215,
      0,
    ]);
    part(standingLegs, new BoxGeometry(0.2, 0.06, 0.28), MARSHAL_SANDALS, [
      side * 0.15,
      0.03,
      0.04,
    ]);
    part(seatedLegs, new BoxGeometry(0.13, 0.13, 0.32), MARSHAL_SKIN.mid, [
      side * 0.25,
      0.275,
      0.16,
    ]);
    part(seatedLegs, new BoxGeometry(0.13, 0.24, 0.13), MARSHAL_SKIN.mid, [side * 0.27, 0.17, 0.3]);
    part(seatedLegs, new BoxGeometry(0.2, 0.06, 0.28), MARSHAL_SANDALS, [side * 0.27, 0.03, 0.34]);
  }

  part(body, new BoxGeometry(0.66, 0.22, 0.44), MARSHAL_SHORTS, [0, 0.47, 0]);
  const belly = part(body, new SphereGeometry(0.36, 8, 6), MARSHAL_JACKET, [0, 0.82, 0]);
  belly.scale.set(1, 0.75, 0.8);
  // A narrower belly a hair proud of the jacket shows as the vest down its open front.
  const vest = part(body, new SphereGeometry(0.36, 8, 6), MARSHAL_VEST, [0, 0.82, 0.005]);
  vest.scale.set(0.3, 0.73, 0.83);
  // Round the belly, leaving the vest bare.
  const opening = 0.45;
  const reflective = part(
    body,
    new CylinderGeometry(0.345, 0.345, 0.045, 8, 1, true, opening, 2 * Math.PI - 2 * opening),
    MARSHAL_REFLECTIVE,
    [0, 0.72, 0],
  );
  reflective.scale.z = 0.8;
  const towel = part(body, new TorusGeometry(0.2, 0.05, 4, 8), MARSHAL_TOWEL, [0, 1.08, 0]);
  towel.rotation.x = Math.PI / 2;

  const head = part(body, new SphereGeometry(MARSHAL_HEAD_RADIUS_M, 8, 6), MARSHAL_SKIN.mid, [
    0,
    MARSHAL_HEAD_Y_M,
    0,
  ]);
  head.name = "head";
  const headDepth = 0.9;
  const headHeight = 0.97;
  head.scale.set(1, headHeight, headDepth);
  // Where the front of his head is at `x` across and `y` up, so a feature stands on it.
  const faceZ = (x: number, y: number) => {
    const across = x / MARSHAL_HEAD_RADIUS_M;
    const up = (y - MARSHAL_HEAD_Y_M) / (MARSHAL_HEAD_RADIUS_M * headHeight);
    return MARSHAL_HEAD_RADIUS_M * headDepth * Math.sqrt(Math.max(0, 1 - across ** 2 - up ** 2));
  };
  const feature = (
    parent: Group,
    [width, height]: readonly [number, number],
    colour: number,
    [x, y]: readonly [number, number],
    tilt = 0,
  ) => {
    const piece = part(parent, new BoxGeometry(width, height, 0.05), colour, [x, y, faceZ(x, y)]);
    piece.rotation.z = tilt;
    // Turned to follow the head round.
    piece.rotation.y = Math.asin(Math.min(1, x / MARSHAL_HEAD_RADIUS_M)) * 0.8;
    return piece;
  };
  const shine = part(body, new SphereGeometry(0.1, 6, 4), MARSHAL_SKIN.lit, [-0.1, 1.66, 0.15]);
  shine.scale.set(1, 0.45, 0.6);
  for (const side of [-1, 1]) {
    part(body, new SphereGeometry(0.07, 6, 4), MARSHAL_SKIN.shaded, [side * 0.34, 1.4, 0]);
    part(body, new BoxGeometry(0.08, 0.2, 0.26), MARSHAL_HAIR, [side * 0.32, 1.44, -0.03]);
    feature(body, [0.22, 0.07], MARSHAL_MUSTACHE, [side * 0.105, 1.305]);
    // The towel's ends down his chest.
    part(body, new BoxGeometry(0.08, 0.24, 0.03), MARSHAL_TOWEL, [side * 0.16, 0.93, 0.25]);
  }
  feature(body, [0.08, 0.08], MARSHAL_NOSE, [0, 1.37]);

  // Calm: flat brows and dot eyes.
  const calm = new Group();
  calm.name = "face.calm";
  // Panic: brows high and lifted at the middle, eyes wide, mouth open and sweat flying off.
  const panic = new Group();
  panic.name = "face.panic";
  body.add(calm, panic);
  for (const side of [-1, 1]) {
    feature(calm, [0.16, 0.045], MARSHAL_BROWS, [side * 0.14, 1.51]);
    feature(calm, [0.06, 0.06], MARSHAL_EYE, [side * 0.13, 1.42]);
    feature(panic, [0.18, 0.05], MARSHAL_BROWS, [side * 0.14, 1.61], -side * 0.2);
    feature(panic, [0.13, 0.13], WHITE.lit, [side * 0.13, 1.43]);
    const pupil = feature(panic, [0.04, 0.04], MARSHAL_EYE, [side * 0.13, 1.43]);
    pupil.position.z += 0.02;
  }
  const mouth = feature(panic, [0.14, 0.16], MARSHAL_MOUTH, [0, 1.23]);
  const sweat = [-1, 1].map((side) => ({
    side,
    drop: part(panic, new SphereGeometry(0.04, 4, 3), MARSHAL_SWEAT),
  }));

  // Each arm hangs from its shoulder along its -y, turned to point where the pose puts the hand.
  const [shoulderX, shoulderY] = MARSHAL_SHOULDER;
  const reach = Math.hypot(MARSHAL_HAND_UP[0] - shoulderX, MARSHAL_HAND_UP[1] - shoulderY);
  const arms = [-1, 1].map((side) => {
    const arm = new Group();
    arm.position.set(side * shoulderX, shoulderY, 0);
    part(arm, new BoxGeometry(0.12, reach, 0.12), MARSHAL_JACKET, [0, -reach / 2, 0]);
    part(arm, new SphereGeometry(0.065, 6, 4), MARSHAL_SKIN.mid, [0, -reach, 0]);
    body.add(arm);
    const up = new Vector3(
      side * (MARSHAL_HAND_UP[0] - shoulderX),
      MARSHAL_HAND_UP[1] - shoulderY,
      0,
    );
    // Seated, the left hand rests on his knee and the right holds the cup to his mouth.
    const seated = side < 0 ? new Vector3(-0.11, -0.38, 0.25) : new Vector3(-0.12, 0.16, 0.38);
    return { arm, up: up.normalize(), seated: seated.normalize() };
  });
  const cup = new Group();
  cup.name = "cup";
  part(cup, new CylinderGeometry(0.06, 0.05, 0.12, 6), MARSHAL_TOWEL, [0.15, 1.2, 0.45]);
  const steam = Array.from({ length: 3 }, () => {
    const puff = part(cup, new SphereGeometry(0.035, 4, 3), MARSHAL_TOWEL);
    puff.material.transparent = true;
    puff.material.depthWrite = false;
    return puff;
  });
  body.add(cup);

  const stool = new Group();
  stool.name = "stool";
  const seatTop = 0.36 - MARSHAL_SEATED_DROP_M;
  part(stool, new BoxGeometry(0.5, 0.05, 0.36), MARSHAL_STOOL, [0, seatTop - 0.025, -0.05]);
  // Its legs cross under the seat, front and back.
  const span = 0.44;
  const rise = seatTop - 0.05;
  for (const z of [-0.2, 0.1]) {
    for (const lean of [-1, 1]) {
      const leg = part(stool, new BoxGeometry(Math.hypot(span, rise), 0.03, 0.03), ROCKET_STRAP, [
        0,
        rise / 2,
        z,
      ]);
      leg.rotation.z = lean * Math.atan2(rise, span);
    }
  }
  figure.add(stool);

  // Each flag hangs in a holder its tilt turns, the flag inside flying toward its own side.
  const flags = [-1, 1].map((side) => {
    const flag = hazardFlag();
    flag.mesh.scale.x = side;
    const holder = new Group();
    holder.name = side < 0 ? "flag.left" : "flag.right";
    holder.add(flag.mesh);
    figure.add(holder);
    return { side, flag, holder };
  });

  const marshal: Marshal = {
    figure,
    pose(kind, clockSeconds) {
      const alarm = kind !== "quiet";
      const hop =
        kind === "rolling"
          ? MARSHAL_HOP_M * Math.abs(Math.sin(clockSeconds * MARSHAL_HOP_RAD_PER_S))
          : 0;
      body.position.y = (alarm ? 0 : -MARSHAL_SEATED_DROP_M) + hop;
      standingLegs.position.y = hop;
      standingLegs.visible = alarm;
      seatedLegs.visible = !alarm;
      stool.visible = !alarm;
      cup.visible = !alarm;
      calm.visible = !alarm;
      panic.visible = alarm;
      for (const { arm, up, seated } of arms) {
        arm.quaternion.setFromUnitVectors(DOWN, alarm ? up : seated);
      }
      flags.forEach(({ side, flag, holder }, index) => {
        if (alarm) {
          // Out of phase: the right is half a wave behind the left.
          const swing = Math.sin(clockSeconds * FLAG_WAVE_RAD_PER_S + (side > 0 ? Math.PI : 0));
          holder.position.set(
            side * MARSHAL_HAND_UP[0],
            MARSHAL_HAND_UP[1] + body.position.y,
            0.05,
          );
          holder.rotation.z = -side * (FLAG_WAVE_OUT_RAD + FLAG_WAVE_SWING_RAD * swing);
          // The two ripple a step apart, so they never fly as one.
          flag.fly(clockSeconds + side);
        } else {
          // Leaning against the stool's left end.
          holder.position.set(-0.62 + index * 0.12, 0.02, -0.15);
          holder.rotation.z = ((12 - index * 6) * Math.PI) / 180;
          flag.furl();
        }
      });
      if (alarm) {
        mouth.scale.y = 1 + 0.12 * Math.sin(clockSeconds * MARSHAL_CHATTER_RAD_PER_S);
        for (const { side, drop } of sweat) {
          const flown = (clockSeconds * MARSHAL_SWEAT_PER_S + (side > 0 ? 0.5 : 0)) % 1;
          drop.position.set(side * (0.4 + flown * 0.2), 1.62 + flown * 0.08, 0);
          drop.scale.setScalar(1 - flown);
        }
      } else {
        steam.forEach((puff, index) => {
          const risen = (clockSeconds * MARSHAL_STEAM_PER_S + index / steam.length) % 1;
          puff.position.set(
            0.15 + Math.sin(clockSeconds * 3 + index * 2) * 0.04,
            1.3 + risen * MARSHAL_STEAM_RISE_M,
            0.45,
          );
          puff.scale.setScalar(1 + risen);
          puff.material.opacity = 0.7 * (1 - risen);
        });
      }
    },
  };
  marshal.pose("quiet", 0);
  return marshal;
}

/** The amber beacon on a marshal post. */
export interface Beacon {
  /** Standing on its origin. */
  readonly mesh: Group;
  /** Light it for the wheel's `kind` while the flag is up, its beam swept side to side `clockSeconds` into the View's clock; dark it otherwise. */
  show(kind: Wheel["kind"], clockSeconds: number): void;
}

/**
 * The marshal post's beacon: an amber dome on a black base, lit by the sun
 * and dark while the wheel is quiet. While the flag is up it glows, a halo
 * round it, and a beam either side sweeps side to side. Its dome, halo and beam
 * are named `dome`, `glow` and `beam`.
 */
export function marshalBeacon(): Beacon {
  const mesh = new Group();
  const baseHeight = BEACON_RADIUS_M * 0.4;
  part(mesh, new CylinderGeometry(BEACON_RADIUS_M, BEACON_RADIUS_M, baseHeight, 8), INK, [
    0,
    baseHeight / 2,
    0,
  ]);
  const dome = part(
    mesh,
    new SphereGeometry(BEACON_RADIUS_M * 0.8, 8, 4, 0, 2 * Math.PI, 0, Math.PI / 2),
    BEACON_OFF,
    [0, baseHeight, 0],
  );
  dome.name = "dome";
  const glow = glowing(
    mesh,
    new SphereGeometry(BEACON_RADIUS_M * 3, 8, 6),
    BEACON_LIT,
    BEACON_GLOW_OPACITY,
  );
  glow.name = "glow";
  glow.material.blending = AdditiveBlending;
  glow.position.y = baseHeight;
  const beam = new Group();
  beam.name = "beam";
  beam.position.y = baseHeight + BEACON_RADIUS_M * 0.4;
  for (const side of [-1, 1]) {
    // Its tip at the dome, opening out along x.
    const cone = new ConeGeometry(BEACON_RADIUS_M * 2.5, BEACON_BEAM_M, 8, 1, true)
      .translate(0, -BEACON_BEAM_M / 2, 0)
      .rotateZ((side * Math.PI) / 2);
    const ray = glowing(beam, cone, BEACON_BEAM, BEACON_BEAM_OPACITY);
    ray.material.side = DoubleSide;
    ray.material.blending = AdditiveBlending;
  }
  mesh.add(beam);
  const beacon: Beacon = {
    mesh,
    show(kind, clockSeconds) {
      const on = kind !== "quiet";
      dome.material.color.setHex(on ? BEACON_LIT : BEACON_OFF);
      dome.material.emissive.setHex(on ? BEACON_LIT : 0);
      glow.visible = on;
      beam.visible = on;
      if (on) {
        const sweep = Math.sin(clockSeconds * BEACON_BEAM_RAD_PER_S);
        beam.rotation.y = BEACON_BEAM_SWING_RAD * sweep;
        glow.scale.setScalar(1 + Math.abs(sweep) / 3);
      }
    },
  };
  beacon.show("quiet", 0);
  return beacon;
}
