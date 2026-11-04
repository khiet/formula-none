import { Box3, InstancedMesh, Matrix4, Mesh, type MeshBasicMaterial, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize } from "./bridge";
import { kartReadout } from "./kart-readout-fixture";
import { livery } from "./livery";
import { GO_GREEN, LAMP_OFF, POSTER_RED } from "./palette";
import {
  CREW_PER_BOX,
  GREEN_HOLD_S,
  PIT_DRAW_RANGE_M,
  type PitBoxMeshes,
  pitBoxMeshes,
  pitLight,
} from "./pit-box-meshes";
import type { PitBox } from "./track";

const KART: KartSize = { width: 1.2, height: 0.6, length: 2 };
/** Two boxes 5 m apart along -z, facing -z. */
const BOXES: readonly PitBox[] = [
  { position: [10, 0, -40], yaw: 0 },
  { position: [10, 0, -45], yaw: 0 },
];
const AT_THE_BOXES: readonly [number, number, number] = [10, 2, -30];
const FAR_AWAY: readonly [number, number, number] = [10, 2, -30 + PIT_DRAW_RANGE_M + 50];

function stopping(guideLeftS: number, stationarySoFarS = 0): KartReadout {
  return kartReadout({
    stop: { tier: "clean", stationaryS: 2.6, guideLeftS, stationarySoFarS },
  });
}

function lamp(pits: PitBoxMeshes, slot: number, name: "red" | "green"): MeshBasicMaterial {
  const found = pits.mesh.getObjectByName(`pitBox${slot}`)?.getObjectByName(`lamp.${name}`);
  if (!(found instanceof Mesh)) throw new Error(`box ${slot} has a ${name} lamp`);
  return found.material as MeshBasicMaterial;
}

function instanced(pits: PitBoxMeshes, name: string): InstancedMesh {
  const found = pits.mesh.getObjectByName(name);
  if (!(found instanceof InstancedMesh)) throw new Error(`${name} is instanced`);
  return found;
}

/** Where each of box `slot`'s crew stands in the world, or null for one drawn at no size. */
function crewAt(pits: PitBoxMeshes, slot: number): (Vector3 | null)[] {
  const bodies = instanced(pits, "crewBodies");
  const matrix = new Matrix4();
  return Array.from({ length: CREW_PER_BOX }, (_, member) => {
    bodies.getMatrixAt(slot * CREW_PER_BOX + member, matrix);
    const scale = new Vector3().setFromMatrixScale(matrix);
    if (scale.x === 0) return null;
    return new Vector3().setFromMatrixPosition(matrix);
  });
}

describe("the pit light", () => {
  it("reads red through the stop, green for a hold after the kart is let go and off otherwise", () => {
    expect(pitLight(stopping(0.5), null, 10)).toBe("red");
    expect(pitLight(stopping(0, 1.5), null, 10)).toBe("red");
    expect(pitLight(kartReadout(), 10, 10 + GREEN_HOLD_S / 2)).toBe("green");
    expect(pitLight(kartReadout(), 10, 10 + GREEN_HOLD_S)).toBe("off");
    expect(pitLight(kartReadout(), null, 10)).toBe("off");
    expect(pitLight(undefined, null, 10)).toBe("off");
  });
});

describe("the pit boxes", () => {
  it("paints each box's mark in its slot's key colour round a kart's footprint, with a gantry light", () => {
    const pits = pitBoxMeshes(BOXES, KART);
    BOXES.forEach((box, slot) => {
      const frame = pits.mesh.getObjectByName(`pitBox${slot}`);
      if (frame === undefined) throw new Error("a frame per box");
      const marks = frame.children.filter((child) => child.name === "mark");
      expect(marks).toHaveLength(4);
      for (const mark of marks) {
        expect((mark as Mesh).material).toHaveProperty("color.r");
        expect(((mark as Mesh).material as MeshBasicMaterial).color.getHex()).toBe(
          livery(slot).key,
        );
      }
      const bounds = new Box3().setFromObject(frame);
      expect(bounds.max.x - bounds.min.x).toBeGreaterThan(KART.width);
      expect(bounds.max.z - bounds.min.z).toBeGreaterThan(KART.length);
      expect(bounds.getCenter(new Vector3()).z).toBeCloseTo(box.position[2], 0);
      expect(frame.getObjectByName("gantry")).toBeDefined();
    });
  });

  it("lights the box red through its kart's stop and green as the kart is let go", () => {
    const pits = pitBoxMeshes(BOXES, KART);
    const off = LAMP_OFF.mid;
    pits.show([kartReadout(), kartReadout()], AT_THE_BOXES, 0.1);
    expect(lamp(pits, 0, "red").color.getHex()).toBe(off);
    expect(lamp(pits, 0, "green").color.getHex()).toBe(off);
    pits.show([stopping(0.3), kartReadout()], AT_THE_BOXES, 0.1);
    expect(lamp(pits, 0, "red").color.getHex()).toBe(POSTER_RED.lit);
    expect(lamp(pits, 0, "green").color.getHex()).toBe(off);
    expect(lamp(pits, 1, "red").color.getHex()).toBe(off);
    pits.show([kartReadout(), kartReadout()], AT_THE_BOXES, 0.1);
    expect(lamp(pits, 0, "red").color.getHex()).toBe(off);
    expect(lamp(pits, 0, "green").color.getHex()).toBe(GO_GREEN.lit);
    pits.show([kartReadout(), kartReadout()], AT_THE_BOXES, GREEN_HOLD_S);
    expect(lamp(pits, 0, "green").color.getHex()).toBe(off);
  });

  it("stands six crew in a row beside each box, in the slot's overalls, and sends them to the wheels while the kart is on its mark", () => {
    const pits = pitBoxMeshes(BOXES, KART);
    const bodies = instanced(pits, "crewBodies");
    expect(bodies.count).toBe(BOXES.length * CREW_PER_BOX);
    expect(instanced(pits, "crewHelmets").count).toBe(BOXES.length * CREW_PER_BOX);
    pits.show([stopping(0.5), kartReadout()], AT_THE_BOXES, 0.1);
    const waiting = crewAt(pits, 0);
    for (const member of waiting) {
      if (member === null) throw new Error("drawn near the camera");
      // Well off the mark, to its right.
      expect(member.x - 10).toBeGreaterThan(KART.width);
    }
    // On the mark: four at the wheels, within reach of the kart's corners, and one off each end.
    pits.show([stopping(0, 1), kartReadout()], AT_THE_BOXES, 0.1);
    const working = crewAt(pits, 0);
    const atWheels = working.slice(0, 4);
    for (const gunner of atWheels) {
      if (gunner === null) throw new Error("drawn near the camera");
      expect(Math.abs(gunner.x - 10)).toBeLessThan(KART.width / 2 + 0.6);
      expect(Math.abs(gunner.z + 40)).toBeCloseTo(KART.length / 2, 0);
    }
    const [front, rear] = working.slice(4);
    expect(front?.z).toBeLessThan(-40 - KART.length / 2);
    expect(rear?.z).toBeGreaterThan(-40 + KART.length / 2);
    // The other box's crew waits on.
    for (const member of crewAt(pits, 1)) {
      expect(member?.x).toBeGreaterThan(10 + KART.width);
    }
  });

  it("draws no crew while the camera is out of range of the lane", () => {
    const pits = pitBoxMeshes(BOXES, KART);
    pits.show([stopping(0, 1), kartReadout()], FAR_AWAY, 0.1);
    expect(crewAt(pits, 0).every((member) => member === null)).toBe(true);
    pits.show([stopping(0, 1), kartReadout()], AT_THE_BOXES, 0.1);
    expect(crewAt(pits, 0).every((member) => member !== null)).toBe(true);
  });
});
