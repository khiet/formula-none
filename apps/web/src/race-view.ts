import {
  AmbientLight,
  BoxGeometry,
  Color,
  DirectionalLight,
  GridHelper,
  Mesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Quaternion,
  Scene,
  Vector3,
} from "three";
import { WebGPURenderer } from "three/webgpu";
import type { KartTransform } from "./bridge";

/** Distance behind and above the local kart the chase camera sits. */
const CAMERA_OFFSET = new Vector3(0, 4, 8);
const CAMERA_LOOK_AHEAD = new Vector3(0, 0.5, -4);
const KART_SIZE_M = 1;
const PLANE_SIZE_M = 400;

export interface RaceView {
  /** Move every kart mesh to the transforms the sim reports and follow the local one. */
  update(transforms: readonly KartTransform[], localKart: number): void;
  render(): void;
  dispose(): void;
}

/**
 * The Race Scene's View: a flat plane and one cube per kart, drawn by
 * Three.js from the transforms the bridge reports. Picks WebGPU when the
 * browser has it and WebGL otherwise.
 */
export async function createRaceView(container: HTMLElement, kartCount: number): Promise<RaceView> {
  const renderer = new WebGPURenderer({ antialias: true });
  await renderer.init();
  renderer.setPixelRatio(window.devicePixelRatio);
  renderer.setSize(container.clientWidth, container.clientHeight);
  container.appendChild(renderer.domElement);

  const scene = new Scene();
  scene.background = new Color(0x87ceeb);
  const camera = new PerspectiveCamera(
    60,
    container.clientWidth / container.clientHeight,
    0.1,
    500,
  );

  const plane = new Mesh(
    new PlaneGeometry(PLANE_SIZE_M, PLANE_SIZE_M),
    new MeshStandardMaterial({ color: 0x4a7a3a }),
  );
  plane.rotation.x = -Math.PI / 2;
  scene.add(plane);
  scene.add(new GridHelper(PLANE_SIZE_M, PLANE_SIZE_M / 4, 0x2f4f2f, 0x2f4f2f));

  const sun = new DirectionalLight(0xffffff, 2);
  sun.position.set(20, 40, 10);
  scene.add(sun, new AmbientLight(0xffffff, 0.6));

  const karts = Array.from({ length: kartCount }, (_, slot) => {
    const kart = new Mesh(
      new BoxGeometry(KART_SIZE_M, KART_SIZE_M, KART_SIZE_M),
      new MeshStandardMaterial({ color: new Color().setHSL(slot / 8, 0.8, 0.5) }),
    );
    scene.add(kart);
    return kart;
  });

  const onResize = () => {
    renderer.setSize(container.clientWidth, container.clientHeight);
    camera.aspect = container.clientWidth / container.clientHeight;
    camera.updateProjectionMatrix();
  };
  window.addEventListener("resize", onResize);

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
      renderer.render(scene, camera);
    },
    dispose() {
      window.removeEventListener("resize", onResize);
      renderer.dispose();
      renderer.domElement.remove();
    },
  };
}
