// The rain as the View draws it (ADR 0103): clouds gathering on the skyline, the
// sky greying, the asphalt darkening with a sheen, a plume at each kart's rear
// wheels and streaks round the camera. All of it follows the look, so a dry race
// draws none of it. The sky's light is the View's light scale (ADR 0098), which
// the look scales through the presentation blend, so nothing here can flash.

import {
  BoxGeometry,
  Color,
  DirectionalLight,
  Group,
  IcosahedronGeometry,
  Mesh,
  MeshBasicMaterial,
  MeshStandardMaterial,
  type Object3D,
  SphereGeometry,
  type Scene as ThreeScene,
} from "three";
import type { KartReadout, KartSize } from "./bridge";
import type { CameraPose } from "./chase-camera";
import type { RainLook } from "./rain-look";
import { createRainSpray, createSprayParticles } from "./rain-spray";
import { createRainStreakParticles, createRainStreaks } from "./rain-streaks";
import { particleMesh } from "./tell-meshes";

const OVERCAST_SKY = 0x8d97a3;
/** The track's material for the road: kerbs, lines and boost pads have their own, and keep their colours. */
const ROAD_MATERIAL = "asphalt";
/** What a wet road's colour is multiplied by, and the roughness its soft sheen comes from. */
const WET_ROAD_SHADE = 0.55;
const WET_ROAD_ROUGHNESS = 0.45;
/** The plume is pale and thin, so a livery reads through it. */
const SPRAY_OPACITY = 0.2;
/** A puff shrinks away over the last share of its life, since one draw call shares one opacity. */
const SPRAY_FADE_SHARE = 0.5;
const STREAK_OPACITY = 0.4;
/**
 * The sheen: a low dim light ahead of the camera, which the wet road's low
 * roughness catches toward the horizon. So low and dim it adds next to nothing to
 * what it lights head on or from above, so kerbs, lines and pads keep their
 * colours, and it moves only with the camera, so it never flashes.
 */
const SHEEN_INTENSITY = 0.3;
const SHEEN_AHEAD_M = 100;
const SHEEN_HEIGHT_M = 14;

/** The most a camera's move between frames reads as a speed: a respawn is a jump, not a drive. */
const CAMERA_SPEED_MOST_M_PER_S = 60;

/** The clouds stand round the camera, this many, this far off and this high, in greys over the skyline. */
const CLOUD_COUNT = 22;
const CLOUD_COLOUR = 0x6f7883;
const CLOUD_RADIUS_MIN_M = 320;
const CLOUD_RADIUS_SPREAD_M = 260;
const CLOUD_HEIGHT_MIN_M = 70;
const CLOUD_HEIGHT_SPREAD_M = 70;
const CLOUD_WIDTH_MIN_M = 70;
const CLOUD_WIDTH_SPREAD_M = 70;
/** How flat a cloud is, and how much of its size it has before it has gathered. */
const CLOUD_FLATTEN = 0.28;
const CLOUD_FIRST_SIZE = 0.35;
const CLOUD_OPACITY = 0.92;

export interface RainView {
  /** Draw `look` for `karts`, seen from `pose`, `frameSeconds` on. */
  update(
    look: RainLook,
    karts: readonly KartReadout[],
    pose: CameraPose,
    frameSeconds: number,
  ): void;
}

/** Everything the rain draws in `world`, over the road materials of `track`, for `kartCount` karts of `size`. */
export function createRainView(
  world: ThreeScene,
  track: Object3D,
  kartCount: number,
  size: KartSize,
  { reducedMotion }: { readonly reducedMotion: boolean },
): RainView {
  const spray = createSprayParticles(kartCount);
  const emitters = Array.from({ length: kartCount }, () => createRainSpray(spray, size));
  const sprayMesh = particleMesh(
    spray,
    new SphereGeometry(0.5, 6, 4),
    new MeshBasicMaterial({ transparent: true, opacity: SPRAY_OPACITY, depthWrite: false }),
    { scaleOf: (puff, life) => puff * Math.min(life / SPRAY_FADE_SHARE, 1) },
  );
  const streaks = createRainStreakParticles();
  const streakEmitter = createRainStreaks(streaks, { reducedMotion });
  const streakMesh = particleMesh(
    streaks,
    new BoxGeometry(1, 1, 1),
    new MeshBasicMaterial({ transparent: true, opacity: STREAK_OPACITY, depthWrite: false }),
    { scaleOf: (thickness) => thickness, streak: true },
  );
  const clouds = cloudDeck();
  const sheen = new DirectionalLight(0xffffff, 0);
  world.add(sprayMesh.mesh, streakMesh.mesh, clouds.group, sheen, sheen.target);

  const road = roadMaterials(track);
  const clearSky = world.background instanceof Color ? world.background.clone() : new Color();
  const greySky = new Color(OVERCAST_SKY);
  const forward: [number, number, number] = [0, 0, -1];
  let lastX = 0;
  let lastZ = 0;
  return {
    update(look, karts, pose, frameSeconds) {
      if (world.background instanceof Color) {
        world.background.copy(clearSky).lerp(greySky, look.overcast);
      }
      for (const { material, colour, wetColour, roughness } of road) {
        material.color.copy(colour).lerp(wetColour, look.wetness);
        material.roughness = roughness + (WET_ROAD_ROUGHNESS - roughness) * look.wetness;
      }
      clouds.show(look.clouds, pose.position);
      karts.forEach((kart, slot) => {
        emitters[slot]?.emit(kart, look.wetness, frameSeconds);
      });
      const [x, , z] = pose.position;
      const [tx, , tz] = pose.target;
      // Measured here, as the pose is all the View has of the camera; a first frame or a stall reads as still.
      const moved = Math.hypot(x - lastX, z - lastZ);
      const speed =
        frameSeconds > 0 ? Math.min(moved / frameSeconds, CAMERA_SPEED_MOST_M_PER_S) : 0;
      lastX = x;
      lastZ = z;
      const length = Math.hypot(tx - x, tz - z);
      if (length > 1e-6) {
        forward[0] = (tx - x) / length;
        forward[2] = (tz - z) / length;
      }
      // Out in the open only: a roof shuts out the sky the road would mirror.
      sheen.intensity = SHEEN_INTENSITY * look.streaks;
      sheen.target.position.set(x, 0, z);
      sheen.position.set(
        x + forward[0] * SHEEN_AHEAD_M,
        SHEEN_HEIGHT_M,
        z + forward[2] * SHEEN_AHEAD_M,
      );
      streakEmitter.emit(pose.position, forward, speed, look.streaks, frameSeconds);
      spray.update(frameSeconds);
      streaks.update(frameSeconds);
      sprayMesh.update();
      streakMesh.update();
    },
  };
}

interface RoadMaterial {
  readonly material: MeshStandardMaterial;
  readonly colour: Color;
  readonly wetColour: Color;
  readonly roughness: number;
}

/** Each road material in `track` with its dry look, once however many meshes share it. */
function roadMaterials(track: Object3D): RoadMaterial[] {
  const found = new Set<MeshStandardMaterial>();
  track.traverse((node) => {
    if (!(node instanceof Mesh)) return;
    const materials: unknown[] = Array.isArray(node.material) ? node.material : [node.material];
    for (const material of materials) {
      if (material instanceof MeshStandardMaterial && material.name === ROAD_MATERIAL) {
        found.add(material);
      }
    }
  });
  return [...found].map((material) => ({
    material,
    colour: material.color.clone(),
    wetColour: material.color.clone().multiplyScalar(WET_ROAD_SHADE),
    roughness: material.roughness,
  }));
}

/** The cloud deck: faceted grey lumps on the skyline that grow and thicken as they gather. */
function cloudDeck(): {
  readonly group: Group;
  show(gathered: number, camera: readonly [number, number, number]): void;
} {
  const group = new Group();
  group.visible = false;
  const geometry = new IcosahedronGeometry(1, 1);
  const material = new MeshBasicMaterial({
    color: CLOUD_COLOUR,
    transparent: true,
    depthWrite: false,
  });
  const widths: number[] = [];
  for (let cloud = 0; cloud < CLOUD_COUNT; cloud++) {
    // A fixed scatter, so every screen and every race shows the same deck.
    const angle = (cloud / CLOUD_COUNT) * 2 * Math.PI + scatter(cloud, 1) * 0.3;
    const radius = CLOUD_RADIUS_MIN_M + scatter(cloud, 2) * CLOUD_RADIUS_SPREAD_M;
    const mesh = new Mesh(geometry, material);
    mesh.position.set(
      Math.cos(angle) * radius,
      CLOUD_HEIGHT_MIN_M + scatter(cloud, 3) * CLOUD_HEIGHT_SPREAD_M,
      Math.sin(angle) * radius,
    );
    mesh.rotation.y = scatter(cloud, 4) * Math.PI;
    widths.push(CLOUD_WIDTH_MIN_M + scatter(cloud, 5) * CLOUD_WIDTH_SPREAD_M);
    group.add(mesh);
  }
  return {
    group,
    show(gathered, [x, , z]) {
      group.visible = gathered > 0;
      if (!group.visible) return;
      // Round the camera, so they stay on the skyline wherever the kart goes.
      group.position.set(x, 0, z);
      material.opacity = CLOUD_OPACITY * gathered;
      const grown = CLOUD_FIRST_SIZE + (1 - CLOUD_FIRST_SIZE) * gathered;
      group.children.forEach((mesh, cloud) => {
        const width = (widths[cloud] ?? 0) * grown;
        mesh.scale.set(width, width * CLOUD_FLATTEN, width);
      });
    },
  };
}

/** A fixed draw in 0 to 1 for `cloud`'s `part`. */
function scatter(cloud: number, part: number): number {
  const value = Math.sin(cloud * 12.9898 + part * 78.233) * 43758.5453;
  return value - Math.floor(value);
}
