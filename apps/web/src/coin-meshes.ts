// The coins a kart collects (ADR 0056), built in code like the items on the
// road (ADR 0037): a flat-shaded gold octagon standing on its edge with an F
// embossed on each face. The View turns and bobs every coin off its own
// clock, so the coins on the track turn together. The spray is the
// spin-out's loss cue: the View alone throws it, after the sim has already
// made the lost coins vanish, so nothing in it can be picked up.

import { BoxGeometry, CylinderGeometry, Group, Mesh, MeshLambertMaterial, Vector3 } from "three";
import type { Coin } from "./bridge";
import type { Vec3 } from "./kart-frame";
import { COIN_GOLD, COIN_RIM } from "./palette";

const COIN_RADIUS_M = 0.4;
const COIN_THICKNESS_M = 0.12;
const COIN_SIDES = 8;
/** The coin's centre over the asphalt, before the bob. */
const COIN_HEIGHT_M = 0.6;
const COIN_TURN_S = 1;
/** The coin's bob, which the item boxes share so the two rise and fall in step. */
export const COIN_BOB_M = 0.05;
export const COIN_BOB_S = 2;

/** The F's metres per unit of the mockup's 100-unit coin, whose radius is 44 units. */
const F_UNIT_M = COIN_RADIUS_M / 44;
const F_DEPTH_M = 0.02;
/** The F's bars as [x, y, width, height] in mockup units from the face's centre, y up. */
const F_BARS: readonly (readonly [number, number, number, number])[] = [
  [-4, 0, 8, 32],
  [1, 12.5, 18, 7],
  [4, -0.5, 8, 7],
];

/** How long a spin-out's spray flies, and how its coins fly. */
export const COIN_SPRAY_SECONDS = 0.6;
const SPRAY_SCALE = 0.5;
const SPRAY_OUT_M_PER_S = 3;
const SPRAY_UP_M_PER_S = 5;
const SPRAY_GRAVITY_M_PER_S2 = 15;
const SPRAY_TURN_RAD_PER_S = 4 * Math.PI;

function material(colour: number): MeshLambertMaterial {
  return new MeshLambertMaterial({ color: colour, flatShading: true });
}

/**
 * A coin standing on its origin: its body, the group's one child, turns on
 * the vertical through its centre, a vertex of the octagon up as in the
 * mockup, the F reading the right way round from either face.
 */
export function coinMesh(): Group {
  const coin = new Group();
  const body = new Group();
  body.position.y = COIN_HEIGHT_M;
  coin.add(body);
  const rim = material(COIN_RIM);
  const gold = material(COIN_GOLD);
  const prism = new CylinderGeometry(COIN_RADIUS_M, COIN_RADIUS_M, COIN_THICKNESS_M, COIN_SIDES);
  // Its caps, the faces, turned from up and down to front and back.
  prism.rotateX(Math.PI / 2);
  body.add(new Mesh(prism, [rim, gold, gold]));
  for (const side of [1, -1]) {
    for (const [x, y, width, height] of F_BARS) {
      const bar = new Mesh(new BoxGeometry(width * F_UNIT_M, height * F_UNIT_M, F_DEPTH_M), rim);
      bar.position.set(
        side * x * F_UNIT_M,
        y * F_UNIT_M,
        (side * (COIN_THICKNESS_M + F_DEPTH_M)) / 2,
      );
      body.add(bar);
    }
  }
  return coin;
}

/** The track's coins as the View draws them, each shown while it is there to pick up. */
export interface CoinMeshes {
  readonly mesh: Group;
  /** Show each coin whose entry, in the sidecar's order, is true; a missing entry shows. */
  show(present: readonly boolean[]): void;
  /** Turn and bob every coin to where it is `clockSeconds` into the View's clock. */
  turn(clockSeconds: number): void;
}

/** A coin at each of `placed`, sharing one coin's geometries and materials. */
export function coinMeshes(placed: readonly Coin[]): CoinMeshes {
  const mesh = new Group();
  const coin = coinMesh();
  const coins = placed.map(({ position, yaw }) => {
    const clone = coin.clone();
    clone.position.set(...position);
    clone.rotation.y = yaw;
    mesh.add(clone);
    return { clone, yaw };
  });
  return {
    mesh,
    show(present) {
      coins.forEach(({ clone }, index) => {
        clone.visible = present[index] ?? true;
      });
    },
    turn(clockSeconds) {
      const angle = ((clockSeconds % COIN_TURN_S) / COIN_TURN_S) * 2 * Math.PI;
      const height =
        COIN_HEIGHT_M + COIN_BOB_M * Math.sin((clockSeconds / COIN_BOB_S) * 2 * Math.PI);
      for (const { clone, yaw } of coins) {
        const body = clone.children[0];
        if (body === undefined) continue;
        // Less the placement's yaw, so every coin faces the same way.
        body.rotation.y = angle - yaw;
        body.position.y = height;
      }
    },
  };
}

/** A spin-out's lost coins thrown out of the kart. */
export interface CoinSpray {
  readonly mesh: Group;
  /** Throw `count` coins up and out from `at`, spread evenly round it. */
  burst(at: Readonly<Vec3>, count: number): void;
  /** Fly every thrown coin `frameSeconds` on; one older than the spray's time is gone. */
  update(frameSeconds: number): void;
}

export function coinSpray(): CoinSpray {
  const mesh = new Group();
  const coin = coinMesh();
  coin.scale.setScalar(SPRAY_SCALE);
  const thrown: { coin: Group; velocity: Vector3; age: number }[] = [];
  return {
    mesh,
    burst([x, y, z], count) {
      for (let k = 0; k < count; k++) {
        let entry = thrown.find((candidate) => !candidate.coin.visible);
        if (entry === undefined) {
          entry = { coin: coin.clone(), velocity: new Vector3(), age: 0 };
          mesh.add(entry.coin);
          thrown.push(entry);
        }
        const angle = (k / count) * 2 * Math.PI;
        entry.coin.position.set(x, y, z);
        entry.coin.visible = true;
        entry.age = 0;
        entry.velocity.set(
          Math.sin(angle) * SPRAY_OUT_M_PER_S,
          SPRAY_UP_M_PER_S,
          Math.cos(angle) * SPRAY_OUT_M_PER_S,
        );
      }
    },
    update(frameSeconds) {
      for (const entry of thrown) {
        if (!entry.coin.visible) continue;
        entry.age += frameSeconds;
        if (entry.age >= COIN_SPRAY_SECONDS) {
          entry.coin.visible = false;
          continue;
        }
        entry.velocity.y -= SPRAY_GRAVITY_M_PER_S2 * frameSeconds;
        entry.coin.position.addScaledVector(entry.velocity, frameSeconds);
        entry.coin.rotation.y += SPRAY_TURN_RAD_PER_S * frameSeconds;
      }
    },
  };
}
