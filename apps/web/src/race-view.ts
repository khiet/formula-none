import {
  AmbientLight,
  Box3,
  BoxGeometry,
  type BufferGeometry,
  Color,
  ConeGeometry,
  DirectionalLight,
  Group,
  type Material,
  Mesh,
  MeshBasicMaterial,
  type Object3D,
  PerspectiveCamera,
  Quaternion,
  SphereGeometry,
  Scene as ThreeScene,
  Vector3,
} from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { CSS2DObject, CSS2DRenderer } from "three/addons/renderers/CSS2DRenderer.js";
import { WebGPURenderer } from "three/webgpu";
import type { Hazard, ItemBox, KartSize, KartTransform, Projectile } from "./bridge";
import type { CameraPose } from "./chase-camera";
import { type DustTell, dustTell } from "./dust";
import type { KartLabel } from "./kart-labels";
import { dressKart, fitKart, type KartFit } from "./kart-model";
import { livery, liveryCss } from "./livery";
import { type SparkTell, sparkTell } from "./sparks";
import { spinOutAngle } from "./spin";

/** Far enough to see across Brands Patch from any point on it. */
const CAMERA_FAR_M = 1000;
/** The placeholder spark: one ball at each rear wheel, this fraction of the kart's height across. */
const SPARK_RADIUS = 0.25;
/** The placeholder dust puff: one ball behind the kart, this fraction of the kart's height across. */
const DUST_RADIUS = 0.6;
/** The placeholder item box: a cube this many metres across, floating this high, in this colour. */
const ITEM_BOX_SIZE_M = 1;
const ITEM_BOX_HEIGHT_M = 1;
const ITEM_BOX_COLOUR = 0xffcc33;
/** The placeholder hazard (ADR 0007): a cone. */
const HAZARD_CONE_RADIUS_M = 0.5;
const HAZARD_CONE_HEIGHT_M = 0.6;
const HAZARD_COLOUR = 0xff5522;
/** The placeholder projectile (ADR 0007): a ball, flying this high over where the sim has it. */
const PROJECTILE_RADIUS_M = 0.4;
const PROJECTILE_HEIGHT_M = 0.6;
const PROJECTILE_COLOUR = 0xdd2222;

export interface RaceView {
  /**
   * Move every kart mesh to the transforms the sim reports, spinning one
   * that is spun out, hang `labels` where they say and hide every other
   * kart's, move the camera to `camera`, show each item box that is there
   * to pick up, a marker at each hazard and a ball at each projectile.
   */
  update(
    transforms: readonly KartTransform[],
    labels: readonly KartLabel[],
    camera: CameraPose,
    itemBoxesPresent: readonly boolean[],
    hazards: readonly Hazard[],
    projectiles: readonly Projectile[],
  ): void;
  render(): void;
  /** Take the canvas off the page and free the renderer. */
  dispose(): void;
}

/** The glTF files the View draws: the track and the kart every slot is a copy of. */
export interface ViewAssets {
  readonly trackGltfUrl: string;
  readonly kartGltfUrl: string;
}

/**
 * The Race Scene's View: the track's glTF, the kart's glTF once per slot in
 * the slot's livery and a box per item box, drawn by Three.js from the
 * transforms the bridge reports and the pose the chase camera computes,
 * with a name floating over each kart the Scene labels, a cone at each
 * hazard and a ball at each projectile. The track mesh is Presentation
 * only; the sim has its own copy of the geometry from the sidecar. A
 * spun-out kart is turned about its axis over `spinOutSeconds`, the
 * spin-out's Feedback. Picks WebGPU when the browser has it and WebGL
 * otherwise.
 */
export async function createRaceView(
  container: HTMLElement,
  kartCount: number,
  kartSize: KartSize,
  assets: ViewAssets,
  itemBoxes: readonly ItemBox[],
  spinOutSeconds: number,
): Promise<RaceView> {
  const renderer = new WebGPURenderer({ antialias: true });
  await renderer.init();
  renderer.setPixelRatio(window.devicePixelRatio);
  renderer.setSize(container.clientWidth, container.clientHeight);
  container.appendChild(renderer.domElement);
  // The labels are page text laid over the canvas where the camera sees their anchors.
  const labelRenderer = new CSS2DRenderer();
  labelRenderer.setSize(container.clientWidth, container.clientHeight);
  labelRenderer.domElement.dataset.kartLabels = "";
  container.appendChild(labelRenderer.domElement);

  // Three's scene graph, not a glossary Scene: this View belongs to the Race Scene.
  const world = new ThreeScene();
  world.background = new Color(0x87ceeb);
  const camera = new PerspectiveCamera(
    60,
    container.clientWidth / container.clientHeight,
    0.1,
    CAMERA_FAR_M,
  );

  const loader = new GLTFLoader();
  const [track, kartModel] = await Promise.all([
    loader.loadAsync(assets.trackGltfUrl),
    loader.loadAsync(assets.kartGltfUrl),
  ]);
  world.add(track.scene);

  const sun = new DirectionalLight(0xffffff, 2);
  sun.position.set(20, 40, 10);
  world.add(sun, new AmbientLight(0xffffff, 0.6));

  const box = new Box3().setFromObject(kartModel.scene);
  const fit = fitKart({ min: box.min.toArray(), max: box.max.toArray() }, kartSize);
  const karts = Array.from({ length: kartCount }, (_, slot) => {
    const kart = kartPresentation(dressKart(kartModel.scene, livery(slot).hex), fit, kartSize);
    world.add(kart.mesh);
    return kart;
  });

  // Unlit so a box reads from any angle, whatever the sun does.
  const boxGeometry = new BoxGeometry(ITEM_BOX_SIZE_M, ITEM_BOX_SIZE_M, ITEM_BOX_SIZE_M);
  const boxMaterial = new MeshBasicMaterial({ color: ITEM_BOX_COLOUR });
  const boxes = itemBoxes.map((itemBox) => {
    const mesh = new Mesh(boxGeometry, boxMaterial);
    const [x, y, z] = itemBox.position;
    mesh.position.set(x, y + ITEM_BOX_HEIGHT_M, z);
    mesh.rotation.y = itemBox.yaw;
    world.add(mesh);
    return mesh;
  });

  const cones = meshPool(
    world,
    new ConeGeometry(HAZARD_CONE_RADIUS_M, HAZARD_CONE_HEIGHT_M),
    new MeshBasicMaterial({ color: HAZARD_COLOUR }),
    HAZARD_CONE_HEIGHT_M / 2,
  );
  const balls = meshPool(
    world,
    new SphereGeometry(PROJECTILE_RADIUS_M),
    new MeshBasicMaterial({ color: PROJECTILE_COLOUR }),
    PROJECTILE_HEIGHT_M,
  );

  const labels = Array.from({ length: kartCount }, (_, slot) => {
    const element = document.createElement("div");
    element.className = "kart-label";
    // A slot's livery is its label's stripe, so the name and the paint are read together.
    element.style.borderColor = liveryCss(slot);
    const label = new CSS2DObject(element);
    label.visible = false;
    world.add(label);
    return label;
  });

  const resize = () => {
    renderer.setSize(container.clientWidth, container.clientHeight);
    labelRenderer.setSize(container.clientWidth, container.clientHeight);
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
  };
  window.addEventListener("resize", resize);

  const target = new Vector3();
  const spin = new Quaternion();
  const up = new Vector3(0, 1, 0);
  return {
    update(transforms, kartLabels, pose, itemBoxesPresent, hazards, projectiles) {
      boxes.forEach((mesh, box) => {
        mesh.visible = itemBoxesPresent[box] ?? true;
      });
      transforms.forEach((transform, slot) => {
        const kart = karts[slot];
        if (kart === undefined) {
          return;
        }
        kart.mesh.position.set(...transform.position);
        kart.mesh.quaternion.set(...transform.rotation);
        if (transform.spinOutSeconds !== null) {
          kart.mesh.quaternion.multiply(
            spin.setFromAxisAngle(up, spinOutAngle(transform.spinOutSeconds, spinOutSeconds)),
          );
        }
        kart.showSparks(sparkTell(transform.drift));
        kart.showDust(dustTell(transform));
      });
      cones.show(hazards);
      balls.show(projectiles);
      for (const label of labels) {
        label.visible = false;
      }
      for (const { kart, text, anchor } of kartLabels) {
        const label = labels[kart];
        if (label === undefined) {
          continue;
        }
        label.visible = true;
        label.position.set(...anchor);
        label.element.textContent = text;
      }
      camera.position.set(...pose.position);
      camera.lookAt(target.set(...pose.target));
    },
    render: () => {
      renderer.render(world, camera);
      labelRenderer.render(world, camera);
    },
    dispose() {
      window.removeEventListener("resize", resize);
      renderer.dispose();
      renderer.domElement.remove();
      labelRenderer.domElement.remove();
    },
  };
}

interface MeshPool {
  /** One mesh at each of `placed`, `lift` metres over its position; the rest hidden. */
  show(placed: readonly { readonly position: readonly [number, number, number] }[]): void;
}

/**
 * Meshes of one shape in `world`, made as they are needed and hidden,
 * never removed, when not, so a race of many never churns the scene graph.
 */
function meshPool(
  world: ThreeScene,
  geometry: BufferGeometry,
  material: Material,
  lift: number,
): MeshPool {
  const meshes: Mesh[] = [];
  return {
    show(placed) {
      while (meshes.length < placed.length) {
        const mesh = new Mesh(geometry, material);
        world.add(mesh);
        meshes.push(mesh);
      }
      meshes.forEach((mesh, index) => {
        const item = placed[index];
        mesh.visible = item !== undefined;
        if (item !== undefined) {
          const [x, y, z] = item.position;
          mesh.position.set(x, y + lift, z);
        }
      });
    },
  };
}

interface KartPresentation {
  readonly mesh: Group;
  /** Show the spark tell at the rear wheels in its colour, or hide it for `null`. */
  showSparks(tell: SparkTell | null): void;
  /** Show the dust tell behind the kart in its colour, or hide it for `null`. */
  showDust(tell: DustTell | null): void;
}

/**
 * One kart on the grid: `model`, already in its livery, fitted into the box
 * the sim collides with, a spark ball at each rear corner of that box for
 * the drift's Feedback and a dust ball behind it for the run-off's. The
 * balls are the placeholder tells (ADR 0007) until the mechanic tells land.
 */
function kartPresentation(model: Object3D, fit: KartFit, size: KartSize): KartPresentation {
  const kart = new Group();
  model.scale.setScalar(fit.scale);
  model.position.set(0, fit.groundY, fit.centreZ);
  kart.add(model);

  // Unlit so the colour reads as the tier whatever the sun does.
  const sparkMaterial = new MeshBasicMaterial({ color: 0xffffff });
  const sparks = new Group();
  const geometry = new SphereGeometry(size.height * SPARK_RADIUS);
  for (const side of [-1, 1]) {
    const spark = new Mesh(geometry, sparkMaterial);
    // Centred on each rear bottom corner of the box, where a wheel would be.
    spark.position.set((side * size.width) / 2, -size.height / 2, size.length / 2);
    sparks.add(spark);
  }
  sparks.visible = false;
  kart.add(sparks);

  const dustMaterial = new MeshBasicMaterial({ color: 0xffffff });
  const dust = new Mesh(new SphereGeometry(size.height * DUST_RADIUS), dustMaterial);
  // Low behind the rear bumper, where the wheels would throw it.
  dust.position.set(0, -size.height / 2, size.length / 2 + size.height * DUST_RADIUS);
  dust.visible = false;
  kart.add(dust);
  return {
    mesh: kart,
    showSparks(tell) {
      sparks.visible = tell !== null;
      if (tell !== null) {
        sparkMaterial.color.setHex(tell.colour);
      }
    },
    showDust(tell) {
      dust.visible = tell !== null;
      if (tell !== null) {
        dustMaterial.color.setHex(tell.colour);
      }
    },
  };
}
