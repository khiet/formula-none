import {
  AmbientLight,
  BoxGeometry,
  Color,
  DirectionalLight,
  Mesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  Quaternion,
  Scene as ThreeScene,
  Vector3,
} from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { WebGPURenderer } from "three/webgpu";
import type { KartTransform } from "./bridge";

/** Distance behind and above the local kart the chase camera sits. */
const CAMERA_OFFSET = new Vector3(0, 4, 8);
const CAMERA_LOOK_AHEAD = new Vector3(0, 0.5, -4);
/** Far enough to see across Brands Patch from any point on it. */
const CAMERA_FAR_M = 1000;
const KART_SIZE_M = 1;
/** Liveries tell karts apart; eight, assigned in grid order until rooms assign them. */
const LIVERIES = 8;

export interface RaceView {
  /** Move every kart mesh to the transforms the sim reports and follow the local one. */
  update(transforms: readonly KartTransform[], localKart: number): void;
  render(): void;
}

/**
 * The Race Scene's View: the track's glTF and one cube per kart, drawn by
 * Three.js from the transforms the bridge reports. The track mesh is
 * Presentation only; the sim has its own copy of the geometry from the
 * sidecar. Picks WebGPU when the browser has it and WebGL otherwise.
 */
export async function createRaceView(
  container: HTMLElement,
  kartCount: number,
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
    const kart = new Mesh(
      new BoxGeometry(KART_SIZE_M, KART_SIZE_M, KART_SIZE_M),
      new MeshStandardMaterial({ color: new Color().setHSL(slot / LIVERIES, 0.8, 0.5) }),
    );
    world.add(kart);
    return kart;
  });

  window.addEventListener("resize", () => {
    renderer.setSize(container.clientWidth, container.clientHeight);
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
  });

  const rotation = new Quaternion();
  const offset = new Vector3();
  const lookAt = new Vector3();
  return {
    update(transforms, localKart) {
      transforms.forEach((transform, slot) => {
        karts[slot]?.position.set(...transform.position);
        karts[slot]?.quaternion.set(...transform.rotation);
      });
      const local = transforms[localKart];
      if (local) {
        rotation.set(...local.rotation);
        offset.copy(CAMERA_OFFSET).applyQuaternion(rotation);
        camera.position.set(...local.position).add(offset);
        lookAt.copy(CAMERA_LOOK_AHEAD).applyQuaternion(rotation).add(camera.position).sub(offset);
        camera.lookAt(lookAt);
      }
    },
    render: () => {
      renderer.render(world, camera);
    },
  };
}
