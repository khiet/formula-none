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
import { createDustEmitter } from "./dust";
import type { KartLabel } from "./kart-labels";
import { dressKart, fitKart, type KartFit } from "./kart-model";
import { livery, liveryCss } from "./livery";
import { createParticles } from "./particles";
import { createSparkEmitter } from "./sparks";
import { spinOutAngle } from "./spin";
import { particleMesh, tyreMarkMesh } from "./tell-meshes";
import { createTyreMarks } from "./tyre-marks";

/** Far enough to see across Brands Patch from any point on it. */
const CAMERA_FAR_M = 1000;
/**
 * Particles live at once across the grid: eight karts' worth of sparks at
 * both wheels through their quarter-second life, and of puffs through
 * theirs, with room over. A pool full earlier drops its oldest.
 */
const SPARK_CAPACITY = 320;
const DUST_CAPACITY = 256;
/** Sparks fall like embers; dust hangs, slows and spreads. */
const SPARK_GRAVITY_M_PER_S2 = 9.8;
const DUST_DRAG_PER_S = 2;
const DUST_GROWTH_PER_S = 2;
const DUST_OPACITY = 0.55;
/** A puff shrinks away over the last share of its life. */
const DUST_FADE_SHARE = 0.4;
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
   * that is spun out, run every kart's tells `frameSeconds` on from what
   * the transforms say, hang `labels` where they say and hide every other
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
    frameSeconds: number,
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
 * only; the sim has its own copy of the geometry from the sidecar. Every
 * mechanic's Feedback is drawn from the transforms alone: tyre marks under
 * a drift, sparks once a tier has charged, dust on the run-off, and a
 * spun-out kart turned about its axis over `spinOutSeconds`. Picks WebGPU
 * when the browser has it and WebGL otherwise.
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
    const kart = kartPresentation(dressKart(kartModel.scene, livery(slot).hex), fit);
    world.add(kart);
    return kart;
  });

  const tyreMarks = createTyreMarks(kartCount, kartSize);
  const marks = tyreMarkMesh(tyreMarks);
  const sparks = createParticles({ capacity: SPARK_CAPACITY, gravity: SPARK_GRAVITY_M_PER_S2 });
  const sparkEmitter = createSparkEmitter(sparks, kartSize);
  // Unlit so the tier's colour reads whatever the sun does; a spark shrinks as it dies.
  const sparkMesh = particleMesh(
    sparks,
    new BoxGeometry(1, 1, 1),
    new MeshBasicMaterial(),
    (size, life) => size * life,
  );
  const dust = createParticles({
    capacity: DUST_CAPACITY,
    drag: DUST_DRAG_PER_S,
    growth: DUST_GROWTH_PER_S,
  });
  const dustEmitter = createDustEmitter(dust, kartSize);
  const dustMesh = particleMesh(
    dust,
    new SphereGeometry(0.5, 6, 4),
    new MeshBasicMaterial({ transparent: true, opacity: DUST_OPACITY, depthWrite: false }),
    (size, life) => size * Math.min(life / DUST_FADE_SHARE, 1),
  );
  world.add(marks.mesh, sparkMesh.mesh, dustMesh.mesh);

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
    update(transforms, kartLabels, pose, itemBoxesPresent, hazards, projectiles, frameSeconds) {
      boxes.forEach((mesh, box) => {
        mesh.visible = itemBoxesPresent[box] ?? true;
      });
      transforms.forEach((transform, slot) => {
        const kart = karts[slot];
        if (kart === undefined) {
          return;
        }
        kart.position.set(...transform.position);
        kart.quaternion.set(...transform.rotation);
        if (transform.spinOutSeconds !== null) {
          kart.quaternion.multiply(
            spin.setFromAxisAngle(up, spinOutAngle(transform.spinOutSeconds, spinOutSeconds)),
          );
        }
        sparkEmitter.emit(transform, frameSeconds);
        dustEmitter.emit(transform, frameSeconds);
      });
      tyreMarks.update(transforms, frameSeconds);
      sparks.update(frameSeconds);
      dust.update(frameSeconds);
      marks.update();
      sparkMesh.update();
      dustMesh.update();
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

/**
 * One kart on the grid: `model`, already in its livery, fitted into the box
 * the sim collides with. Its tells are drawn by the View's shared meshes
 * from the transform, not hung on the kart.
 */
function kartPresentation(model: Object3D, fit: KartFit): Group {
  const kart = new Group();
  model.scale.setScalar(fit.scale);
  model.position.set(0, fit.groundY, fit.centreZ);
  kart.add(model);
  return kart;
}
