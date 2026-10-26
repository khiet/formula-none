import {
  AmbientLight,
  Box3,
  BoxGeometry,
  Color,
  DirectionalLight,
  Group,
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
import type { ItemBox, KartReadout, KartSize } from "./bridge";
import type { CameraPose } from "./chase-camera";
import { createDustEmitter } from "./dust";
import { hazardConeMesh, homingGullMesh, itemBoxMesh } from "./item-meshes";
import type { KartLabel } from "./kart-labels";
import { dressKart, fitKart, type KartFit } from "./kart-model";
import { createKartShadows } from "./kart-shadows";
import { livery } from "./livery";
import { plate, tonesOf, writePalette } from "./palette";
import { createParticles } from "./particles";
import type { RaceReadout } from "./race-run";
import { createSparkEmitter } from "./sparks";
import { spinOutAngle } from "./spin";
import { particleMesh, shadowDiscMesh, tyreMarkMesh } from "./tell-meshes";
import { trackProbe } from "./track-probe";
import { createTyreMarks } from "./tyre-marks";
import { kartWheels } from "./wheel-spin";

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
/** How high an item box floats over where the sim has it. */
const ITEM_BOX_HEIGHT_M = 1;
/** How high the projectile (ADR 0007) flies over where the sim has it. */
const PROJECTILE_HEIGHT_M = 0.6;

export interface RaceView {
  /**
   * Move every kart mesh to the transform `race` reports for it, spinning
   * one that is spun out, run every kart's tells `frameSeconds` on from
   * what the readouts say and the phase (the rocket start's sparks show
   * only while it counts down), hang the labels the karts call for and
   * hide every other kart's, move the camera to `camera`, show each item
   * box that is there to pick up, a cone at each hazard and a gull at
   * each projectile, turned toward the kart it homes in on.
   */
  update(race: RaceReadout, camera: CameraPose, frameSeconds: number): void;
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
 * kart readouts the bridge reports and the pose the chase camera computes,
 * with a name floating over each kart the Scene labels, a cone at each
 * hazard and a gull at each projectile. The track mesh is Presentation
 * only; the sim has its own copy of the geometry from the sidecar. Every
 * mechanic's Feedback is drawn from the readouts and the phase alone:
 * tyre marks under a drift, sparks once a tier has charged, drifting or
 * holding for the rocket start, dust on the run-off and at the wheels of a
 * stalled kart, a spun-out kart turned about its axis over
 * `spinOutSeconds`, and a shadow disc on the track under every kart so a
 * hop's height reads against the ground. `labelsFor` names the karts to
 * label and where, from the frame's readouts. Picks WebGPU when the
 * browser has it and WebGL otherwise.
 */
export async function createRaceView(
  container: HTMLElement,
  kartCount: number,
  kartSize: KartSize,
  assets: ViewAssets,
  itemBoxes: readonly ItemBox[],
  spinOutSeconds: number,
  labelsFor: (karts: readonly KartReadout[]) => readonly KartLabel[],
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
  writePalette(labelRenderer.domElement);
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
    const model = dressKart(kartModel.scene, livery(slot));
    // Measured before the fit scales the model, then drawn at its scale.
    const wheels = kartWheels(model, fit.scale);
    const kart = kartPresentation(model, fit);
    world.add(kart);
    return { kart, wheels };
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
  // The discs find the surface in the View's own track mesh; the bridge
  // offers no surface query and Presentation never asks the sim.
  const shadows = createKartShadows(kartCount, trackProbe(track.scene, kartSize.height / 2));
  const shadowMesh = shadowDiscMesh(shadows, kartSize);
  world.add(marks.mesh, sparkMesh.mesh, dustMesh.mesh, shadowMesh.mesh);

  // Clones share the one box's geometries and materials.
  const itemBox = itemBoxMesh();
  const boxes = itemBoxes.map((placed) => {
    const mesh = itemBox.clone();
    const [x, y, z] = placed.position;
    mesh.position.set(x, y + ITEM_BOX_HEIGHT_M, z);
    mesh.rotation.y = placed.yaw;
    world.add(mesh);
    return mesh;
  });

  const cones = meshPool(world, hazardConeMesh(), 0);
  const gulls = meshPool(world, homingGullMesh(), PROJECTILE_HEIGHT_M);

  const labels = Array.from({ length: kartCount }, (_, slot) => {
    const element = document.createElement("div");
    element.className = "kart-label";
    // The plate is the kart's key colour, so the name and the paint are read together.
    plate(element, tonesOf(livery(slot).key));
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
    update(race, pose, frameSeconds) {
      const { karts: readouts, itemBoxesPresent, hazards, projectiles } = race;
      const countingDown = race.phase.kind === "countdown";
      boxes.forEach((mesh, box) => {
        mesh.visible = itemBoxesPresent[box] ?? true;
      });
      readouts.forEach((readout, slot) => {
        const drawn = karts[slot];
        if (drawn === undefined) {
          return;
        }
        const { kart, wheels } = drawn;
        wheels.turn(readout.speed, frameSeconds);
        kart.position.set(...readout.position);
        kart.quaternion.set(...readout.rotation);
        if (readout.spinOutS !== null) {
          kart.quaternion.multiply(
            spin.setFromAxisAngle(up, spinOutAngle(readout.spinOutS, spinOutSeconds)),
          );
        }
        sparkEmitter.emit(readout, frameSeconds, countingDown);
        dustEmitter.emit(readout, frameSeconds);
      });
      tyreMarks.update(readouts, frameSeconds);
      sparks.update(frameSeconds);
      dust.update(frameSeconds);
      shadows.update(readouts);
      marks.update();
      sparkMesh.update();
      dustMesh.update();
      shadowMesh.update();
      cones.show(hazards);
      gulls.show(projectiles, (projectile) => readouts[projectile.target]?.position);
      for (const label of labels) {
        label.visible = false;
      }
      for (const { kart, text, anchor } of labelsFor(readouts)) {
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

type Position = readonly [number, number, number];

interface MeshPool {
  /**
   * One copy at each of `placed`, `lift` metres over its position and, when
   * `facing` names a point, turned level to face it; the rest hidden.
   */
  show<Placed extends { readonly position: Position }>(
    placed: readonly Placed[],
    facing?: (item: Placed) => Position | undefined,
  ): void;
}

/**
 * Copies of `template` in `world`, sharing its geometries and materials,
 * made as they are needed and hidden, never removed, when not, so a race of
 * many never churns the scene graph.
 */
function meshPool(world: ThreeScene, template: Object3D, lift: number): MeshPool {
  const meshes: Object3D[] = [];
  const toward = new Vector3();
  return {
    show(placed, facing) {
      while (meshes.length < placed.length) {
        const mesh = template.clone();
        world.add(mesh);
        meshes.push(mesh);
      }
      meshes.forEach((mesh, index) => {
        const item = placed[index];
        mesh.visible = item !== undefined;
        if (item !== undefined) {
          const [x, y, z] = item.position;
          mesh.position.set(x, y + lift, z);
          const target = facing?.(item);
          if (target !== undefined) {
            mesh.lookAt(toward.set(target[0], y + lift, target[2]));
          }
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
