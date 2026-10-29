// The hazard zone's flash (GLOSSARY), as docs/mockups/runaway-wheel-danger-spec.html
// draws it: a red wash over the zone painted in the track's glTF, pulsing
// from the flag going up to the end of the pass. Presentation only, keyed
// on the wheel's kind like the marshal's pose.

import { Mesh, MeshBasicMaterial, type Object3D } from "three";
import type { Wheel } from "./bridge";
import { SIGNAL_RED } from "./palette";

/** The wash pulses between these opacities this fast. */
const FLASH_MIN_OPACITY = 0.2;
const FLASH_MAX_OPACITY = 0.6;
const FLASH_RAD_PER_S = 14;

/** How opaque the red wash over a crossing's zone is with its wheel `wheel` and the View's clock at `clockSeconds`: 0 while quiet. */
export function hazardFlashOpacity(wheel: Wheel, clockSeconds: number): number {
  if (wheel.kind === "quiet") return 0;
  const pulse = 0.5 + 0.5 * Math.sin(clockSeconds * FLASH_RAD_PER_S);
  return FLASH_MIN_OPACITY + (FLASH_MAX_OPACITY - FLASH_MIN_OPACITY) * pulse;
}

export interface HazardZones {
  /** Flash each crossing's zone as its wheel in `wheels` says. */
  show(wheels: readonly Wheel[], clockSeconds: number): void;
}

/**
 * A red wash laid over each hazard zone the track's glTF paints, crossing
 * and zone matched by their number: a copy of every piece of the zone's
 * paint, drawn over it. The loader drops the dots from the exported names.
 */
export function hazardZones(track: Object3D): HazardZones {
  const washes = new Map<number, { material: MeshBasicMaterial; meshes: Mesh[] }>();
  const pieces: { number: number; mesh: Mesh }[] = [];
  track.traverse((node) => {
    const match = /^HazardZone\.?(\d+)/.exec(node.name);
    if (match === null) return;
    node.traverse((part) => {
      if (part instanceof Mesh) pieces.push({ number: Number(match[1]), mesh: part });
    });
  });
  for (const { number, mesh } of pieces) {
    let wash = washes.get(number);
    if (wash === undefined) {
      wash = { material: washMaterial(), meshes: [] };
      washes.set(number, wash);
    }
    const copy = new Mesh(mesh.geometry, wash.material);
    copy.visible = false;
    copy.renderOrder = 1;
    mesh.add(copy);
    wash.meshes.push(copy);
  }
  return {
    show(wheels, clockSeconds) {
      wheels.forEach((wheel, crossing) => {
        const wash = washes.get(crossing);
        if (wash === undefined) return;
        const opacity = hazardFlashOpacity(wheel, clockSeconds);
        wash.material.opacity = opacity;
        for (const mesh of wash.meshes) mesh.visible = opacity > 0;
      });
    },
  };
}

/** Drawn over the paint it copies without fighting it for depth. */
function washMaterial(): MeshBasicMaterial {
  return new MeshBasicMaterial({
    color: SIGNAL_RED.mid,
    transparent: true,
    depthWrite: false,
    polygonOffset: true,
    polygonOffsetFactor: -1,
    polygonOffsetUnits: -4,
  });
}
