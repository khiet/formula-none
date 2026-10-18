import {
  AmbientLight,
  BoxGeometry,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  Scene as ThreeScene,
  Vector3,
} from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { WebGPURenderer } from "three/webgpu";
import type { KartSize, KartTransform } from "./bridge";
import type { CameraPose } from "./chase-camera";

/** Far enough to see across Brands Patch from any point on it. */
const CAMERA_FAR_M = 1000;
/** Liveries tell karts apart; eight, assigned in grid order until rooms assign them. */
const LIVERIES = 8;
/** The placeholder kart's cowl as fractions of the kart box, sitting over its rear half. */
const COWL_WIDTH = 0.6;
const COWL_HEIGHT = 0.5;
const COWL_LENGTH = 0.4;
const COWL_COLOUR = 0x222222;

export interface RaceView {
  /** Move every kart mesh to the transforms the sim reports and the camera to `camera`. */
  update(transforms: readonly KartTransform[], camera: CameraPose): void;
  render(): void;
}

/**
 * The Race Scene's View: the track's glTF and one box per kart, drawn by
 * Three.js from the transforms the bridge reports and the pose the chase
 * camera computes. The track mesh is Presentation only; the sim has its own
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
    const kart = placeholderKart(kartSize, new Color().setHSL(slot / LIVERIES, 0.8, 0.5));
    world.add(kart);
    return kart;
  });

  window.addEventListener("resize", () => {
    renderer.setSize(container.clientWidth, container.clientHeight);
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
  });

  const target = new Vector3();
  return {
    update(transforms, pose) {
      transforms.forEach((transform, slot) => {
        karts[slot]?.position.set(...transform.position);
        karts[slot]?.quaternion.set(...transform.rotation);
      });
      camera.position.set(...pose.position);
      camera.lookAt(target.set(...pose.target));
    },
    render: () => {
      renderer.render(world, camera);
    },
  };
}

/**
 * The placeholder kart until the glTF kart lands (ADR 0007): the box the sim
 * collides with in the livery colour, with a dark cowl over the rear half so
 * the facing reads from behind and from the side.
 */
function placeholderKart(size: KartSize, livery: Color): Group {
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
  return kart;
}
