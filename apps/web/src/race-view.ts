import {
  AmbientLight,
  BoxGeometry,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshStandardMaterial,
  PerspectiveCamera,
  SphereGeometry,
  Scene as ThreeScene,
  Vector3,
} from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { CSS2DObject, CSS2DRenderer } from "three/addons/renderers/CSS2DRenderer.js";
import { WebGPURenderer } from "three/webgpu";
import type { KartSize, KartTransform } from "./bridge";
import type { CameraPose } from "./chase-camera";
import { type DustTell, dustTell } from "./dust";
import type { KartLabel } from "./kart-labels";
import { liveryColour } from "./livery";
import { type SparkTell, sparkTell } from "./sparks";

/** Far enough to see across Brands Patch from any point on it. */
const CAMERA_FAR_M = 1000;
/** The placeholder kart's cowl as fractions of the kart box, sitting over its rear half. */
const COWL_WIDTH = 0.6;
const COWL_HEIGHT = 0.5;
const COWL_LENGTH = 0.4;
const COWL_COLOUR = 0x222222;
/** The placeholder spark: one ball at each rear wheel, this fraction of the kart's height across. */
const SPARK_RADIUS = 0.25;
/** The placeholder dust puff: one ball behind the kart, this fraction of the kart's height across. */
const DUST_RADIUS = 0.6;

export interface RaceView {
  /**
   * Move every kart mesh to the transforms the sim reports, hang `labels`
   * where they say and hide every other kart's, and move the camera to
   * `camera`.
   */
  update(
    transforms: readonly KartTransform[],
    labels: readonly KartLabel[],
    camera: CameraPose,
  ): void;
  render(): void;
  /** Take the canvas off the page and free the renderer. */
  dispose(): void;
}

/**
 * The Race Scene's View: the track's glTF and one box per kart, drawn by
 * Three.js from the transforms the bridge reports and the pose the chase
 * camera computes, with a name floating over each kart the Scene labels. The track mesh is Presentation only; the sim has its own
 * copy of the geometry from the sidecar. Picks WebGPU when the browser has
 * it and WebGL otherwise.
 */
export async function createRaceView(
  container: HTMLElement,
  kartCount: number,
  kartSize: KartSize,
  trackGltfUrl: string,
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

  const track = await new GLTFLoader().loadAsync(trackGltfUrl);
  world.add(track.scene);

  const sun = new DirectionalLight(0xffffff, 2);
  sun.position.set(20, 40, 10);
  world.add(sun, new AmbientLight(0xffffff, 0.6));

  const karts = Array.from({ length: kartCount }, (_, slot) => {
    const { hue, saturation, lightness } = liveryColour(slot);
    const kart = placeholderKart(kartSize, new Color().setHSL(hue, saturation, lightness));
    world.add(kart.mesh);
    return kart;
  });

  const labels = Array.from({ length: kartCount }, () => {
    const element = document.createElement("div");
    element.className = "kart-label";
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
  return {
    update(transforms, kartLabels, pose) {
      transforms.forEach((transform, slot) => {
        const kart = karts[slot];
        if (kart === undefined) {
          return;
        }
        kart.mesh.position.set(...transform.position);
        kart.mesh.quaternion.set(...transform.rotation);
        kart.showSparks(sparkTell(transform.drift));
        kart.showDust(dustTell(transform));
      });
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

interface PlaceholderKart {
  readonly mesh: Group;
  /** Show the spark tell at the rear wheels in its colour, or hide it for `null`. */
  showSparks(tell: SparkTell | null): void;
  /** Show the dust tell behind the kart in its colour, or hide it for `null`. */
  showDust(tell: DustTell | null): void;
}

/**
 * The placeholder kart until the glTF kart lands (ADR 0007): the box the sim
 * collides with in the livery colour, with a dark cowl over the rear half so
 * the facing reads from behind and from the side, a spark ball at each
 * rear wheel for the drift's Feedback and a dust ball behind it for the
 * run-off's.
 */
function placeholderKart(size: KartSize, livery: Color): PlaceholderKart {
  const kart = new Group();
  kart.add(
    new Mesh(
      new BoxGeometry(size.width, size.height, size.length),
      new MeshStandardMaterial({ color: livery }),
    ),
  );
  const cowl = new Mesh(
    new BoxGeometry(size.width * COWL_WIDTH, size.height * COWL_HEIGHT, size.length * COWL_LENGTH),
    new MeshStandardMaterial({ color: COWL_COLOUR }),
  );
  // On top of the box, over its rear; the kart faces -Z, so the rear is +Z.
  cowl.position.set(0, size.height * (0.5 + COWL_HEIGHT / 2), (size.length * COWL_LENGTH) / 2);
  kart.add(cowl);

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
