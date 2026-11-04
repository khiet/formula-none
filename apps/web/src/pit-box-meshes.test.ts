import { Box3, InstancedMesh, Matrix4, Mesh, type MeshBasicMaterial, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize, StopTier } from "./bridge";
import { kartReadout } from "./kart-readout-fixture";
import { livery } from "./livery";
import { GO_GREEN, LAMP_OFF, POSTER_RED } from "./palette";
import {
  CREW_PER_BOX,
  crewWork,
  GREEN_HOLD_S,
  PIT_DRAW_RANGE_M,
  type PitBoxMeshes,
  pitBoxMeshes,
  pitLight,
} from "./pit-box-meshes";
import type { PitBox } from "./track";

const KART: KartSize = { width: 1.2, height: 0.6, length: 2 };
/** Two pit boxes 5 m apart along -z, facing -z. */
const BOXES: readonly PitBox[] = [
  { position: [10, 0, -40], yaw: 0, buildingSide: 1 },
  { position: [10, 0, -45], yaw: 0, buildingSide: 1 },
];
const AT_THE_BOXES: readonly [number, number, number] = [10, 2, -30];
const OPTIONS = { cleanStopSeconds: 3 };
const FAR_AWAY: readonly [number, number, number] = [10, 2, -30 + PIT_DRAW_RANGE_M + 50];

function stopping(guideLeftS: number, stationarySoFarS = 0, tier: StopTier = "clean"): KartReadout {
  return kartReadout({
    stop: { tier, stationaryS: 2.6, guideLeftS, stationarySoFarS },
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

/** Where each of pit box `slot`'s crew stands in the world, or null for one drawn at no size. */
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
    expect(pitLight(true, null, 10)).toBe("red");
    expect(pitLight(true, 8, 10)).toBe("red");
    expect(pitLight(false, 10, 10 + GREEN_HOLD_S / 2)).toBe("green");
    expect(pitLight(false, 10, 10 + GREEN_HOLD_S)).toBe("off");
    expect(pitLight(false, null, 10)).toBe("off");
  });
});

describe("the crew's work", () => {
  it("waits through the guide, works through a clean stop's time and then acts the roll out", () => {
    expect(crewWork(null, 3)).toBe("waiting");
    expect(crewWork(stopping(0.4).stop, 3)).toBe("waiting");
    expect(crewWork(stopping(0, 1, "botched").stop, 3)).toBe("working");
    expect(crewWork(stopping(0, 3, "slow").stop, 3)).toBe("working");
    expect(crewWork(stopping(0, 3.5, "clean").stop, 3)).toBe("working");
    expect(crewWork(stopping(0, 3.5, "slow").stop, 3)).toBe("slow");
    expect(crewWork(stopping(0, 3.5, "botched").stop, 3)).toBe("botched");
    expect(crewWork(stopping(0, 9, "disaster").stop, 3)).toBe("disaster");
  });
});

describe("the pit boxes", () => {
  it("paints each box's mark in its slot's key colour round a kart's footprint, with a gantry light", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
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
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
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
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
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
    // The other pit box's crew waits on.
    for (const member of crewAt(pits, 1)) {
      expect(member?.x).toBeGreaterThan(10 + KART.width);
    }
  });

  it("acts the roll out past a clean stop's time: one gunner stays, four swarm a wheel, or the jacks give up", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    const gunnersAtWheels = () =>
      crewAt(pits, 0)
        .slice(0, 4)
        .filter((gunner) => gunner !== null && Math.abs(gunner.x - 10) < KART.width).length;
    pits.show([stopping(0, 4, "slow"), kartReadout()], AT_THE_BOXES, 0.1);
    expect(gunnersAtWheels()).toBe(1);
    pits.show([stopping(0, 4, "botched"), kartReadout()], AT_THE_BOXES, 0.1);
    expect(gunnersAtWheels()).toBe(4);
    const swarm = crewAt(pits, 0).slice(0, 4);
    // All at the rear left wheel, one behind the other.
    for (const gunner of swarm) expect(gunner?.x).toBeLessThan(10);
    expect(new Set(swarm.map((gunner) => gunner?.z.toFixed(2))).size).toBe(4);
    pits.show([stopping(0, 9, "disaster"), kartReadout()], AT_THE_BOXES, 0.1);
    expect(gunnersAtWheels()).toBe(0);
    const [front, rear] = crewAt(pits, 0).slice(4);
    expect(front?.z).toBeLessThan(-40 - KART.length / 2 - 1.5);
    expect(rear?.z).toBeGreaterThan(-40 + KART.length / 2 + 1.5);
  });

  it("draws no crew while the camera is out of range of the lane", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    pits.show([stopping(0, 1), kartReadout()], FAR_AWAY, 0.1);
    expect(crewAt(pits, 0).every((member) => member === null)).toBe(true);
    pits.show([stopping(0, 1), kartReadout()], AT_THE_BOXES, 0.1);
    expect(crewAt(pits, 0).every((member) => member !== null)).toBe(true);
  });
});
