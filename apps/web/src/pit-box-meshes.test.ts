import {
  Box3,
  BufferGeometry,
  Color,
  Group,
  InstancedMesh,
  LineSegments,
  Matrix4,
  Mesh,
  type MeshBasicMaterial,
  MeshStandardMaterial,
  Vector3,
} from "three";
import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize, StopTier } from "./bridge";
import { CREW_PER_BOX, crewRoster } from "./crew-roster";
import { FACE_PARTS } from "./driver-parts";
import { kartReadout } from "./kart-readout-fixture";
import { livery } from "./livery";
import { GO_GREEN, LAMP_OFF, POSTER_RED } from "./palette";
import {
  BOOM_HEIGHT_M,
  PIT_DRAW_RANGE_M,
  type PitBoxMeshes,
  pitBoxMeshes,
  pitLight,
} from "./pit-box-meshes";
import { GREEN_HOLD_S } from "./stop-beats";
import type { PitBox } from "./track";

const KART: KartSize = { width: 1.2, height: 0.6, length: 1.8 };
/** Two pit boxes 5 m apart along -z, facing -z, the building on their +x. */
const BOXES: readonly PitBox[] = [
  { position: [10, 0, -40], yaw: 0, buildingSide: 1 },
  { position: [10, 0, -45], yaw: 0, buildingSide: 1 },
];
const AT_THE_BOXES: readonly [number, number, number] = [10, 2, -30];
const FAR_AWAY: readonly [number, number, number] = [10, 2, -30 + PIT_DRAW_RANGE_M + 50];
const GUNNERS = [2, 3, 4, 5];

/**
 * A stand-in for kart.glb's head: the head itself in skin, and every face
 * part under it in hair, one of them a group of two meshes the way the
 * exporter makes a part in two materials.
 */
function head(): Group {
  const root = new Group();
  root.name = "head";
  const mesh = (name: string, material: string) => {
    const part = new Mesh(new BufferGeometry(), new MeshStandardMaterial({ name: material }));
    part.name = name;
    return part;
  };
  root.add(mesh("head-skin", "Skin"));
  for (const part of FACE_PARTS) {
    if (part === "hair-short-greying-temples") {
      const group = new Group();
      group.name = part;
      group.add(mesh(`${part}-hair`, "Hair"), mesh(`${part}-grey`, "Grey"));
      root.add(group);
    } else {
      root.add(mesh(part, "Hair"));
    }
  }
  return root;
}

const OPTIONS = { cleanStopSeconds: 3, guideSeconds: 0.5, head: head() };

function stopping(
  guideLeftS: number,
  stationarySoFarS = 0,
  tier: StopTier = "clean",
  stationaryS = 2.65,
): KartReadout {
  return kartReadout({ stop: { tier, stationaryS, guideLeftS, stationarySoFarS } });
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

/** The instanced pieces of the head that face part `part` is drawn by. */
function partPieces(pits: PitBoxMeshes, part: string): InstancedMesh[] {
  return pits.mesh.children.filter(
    (child): child is InstancedMesh =>
      child instanceof InstancedMesh && child.name.startsWith(`crew.${part}.`),
  );
}

const drawn = (mesh: InstancedMesh, index: number) => {
  const matrix = new Matrix4();
  mesh.getMatrixAt(index, matrix);
  return new Vector3().setFromMatrixScale(matrix).x > 0;
};

/** Where each of pit box `slot`'s crew has their torso in the world, or null for one drawn at no size. */
function crewAt(pits: PitBoxMeshes, slot: number): (Vector3 | null)[] {
  const suit = instanced(pits, "crewSuit");
  const per = suit.count / (BOXES.length * CREW_PER_BOX);
  const matrix = new Matrix4();
  return Array.from({ length: CREW_PER_BOX }, (_, member) => {
    suit.getMatrixAt((slot * CREW_PER_BOX + member) * per, matrix);
    const scale = new Vector3().setFromMatrixScale(matrix);
    if (scale.x === 0) return null;
    return new Vector3().setFromMatrixPosition(matrix);
  });
}

/** Whether a torso is beside one of box 0's wheels: out from the kart's side at a hub. */
const atAWheel = (at: Vector3 | null) =>
  at !== null &&
  Math.abs(Math.abs(at.x - 10) - 1.1) < 0.3 &&
  Math.abs(Math.abs(at.z + 40) - 0.52) < 0.3;

describe("the pit light", () => {
  it("reads red through the stop, green for a hold after the kart is let go and off otherwise", () => {
    expect(pitLight(true, null, 10)).toBe("red");
    expect(pitLight(true, 8, 10)).toBe("red");
    expect(pitLight(false, 10, 10 + GREEN_HOLD_S / 2)).toBe("green");
    expect(pitLight(false, 10, 10 + GREEN_HOLD_S)).toBe("off");
    expect(pitLight(false, null, 10)).toBe("off");
  });
});

describe("the pit boxes", () => {
  it("paints each box's mark in its slot's key colour round a kart's footprint, with a gantry light and a boom over it", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    pits.mesh.updateMatrixWorld(true);
    BOXES.forEach((box, slot) => {
      const frame = pits.mesh.getObjectByName(`pitBox${slot}`);
      if (frame === undefined) throw new Error("a frame per box");
      const marks = frame.children.filter((child) => child.name === "mark");
      expect(marks).toHaveLength(4);
      for (const mark of marks) {
        expect(((mark as Mesh).material as MeshBasicMaterial).color.getHex()).toBe(
          livery(slot).key,
        );
        expect(mark.position.y).toBeLessThan(0.1);
      }
      expect(frame.getObjectByName("gantry")).toBeDefined();
      const boom = frame.getObjectByName("boom");
      if (boom === undefined) throw new Error("a boom per box");
      const bounds = new Box3().setFromObject(boom);
      expect(bounds.min.y).toBeGreaterThan(BOOM_HEIGHT_M - 0.5);
      expect(bounds.max.x - bounds.min.x).toBeGreaterThan(KART.width);
      expect(bounds.getCenter(new Vector3()).z).toBeCloseTo(box.position[2], 0);
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

  it("draws seven crew per box in the slot's overalls, waiting in a row on the building's side", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    const suit = instanced(pits, "crewSuit");
    pits.show([kartReadout(), kartReadout()], AT_THE_BOXES, 0.1);
    BOXES.forEach((_, slot) => {
      const crew = crewAt(pits, slot);
      expect(crew).toHaveLength(7);
      const per = suit.count / (BOXES.length * CREW_PER_BOX);
      crew.forEach((member, index) => {
        if (member === null) throw new Error("drawn near the camera");
        expect(member.x - 10).toBeGreaterThan(KART.width);
        const colour = new Color();
        suit.getColorAt((slot * CREW_PER_BOX + index) * per, colour);
        expect(colour.getHex()).toBe(livery(slot).key);
      });
    });
  });

  it("gives each crew member the roster's face under a headset", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    pits.show([kartReadout(), kartReadout()], AT_THE_BOXES, 0.1);
    BOXES.forEach((_, slot) => {
      crewRoster(slot).forEach((face, member) => {
        const figure = slot * CREW_PER_BOX + member;
        for (const part of FACE_PARTS) {
          const worn =
            part === `hair-${face.hair}` ||
            part === `facial-hair-${face.facialHair}` ||
            part === `brows-${face.brows}` ||
            part === `nose-${face.nose}` ||
            part === `mouth-${face.mouth}` ||
            part === "headset" ||
            (part === "earrings" && face.earrings) ||
            (part === "glasses" && face.glasses);
          for (const piece of partPieces(pits, part)) expect(drawn(piece, figure)).toBe(worn);
        }
        const skin = partPieces(pits, "head")[0];
        if (skin === undefined) throw new Error("the head itself is drawn");
        expect(drawn(skin, figure)).toBe(true);
        const colour = new Color();
        skin.getColorAt(figure, colour);
        expect(colour.getHex()).toBe(face.skin);
      });
    });
  });

  it("runs the crew out on the catch: a gunner at every wheel, a jack off each end, the holder off the nose", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    pits.show([stopping(0, 0.6), kartReadout()], AT_THE_BOXES, 0.1);
    const crew = crewAt(pits, 0);
    for (const member of GUNNERS) expect(atAWheel(crew[member] ?? null)).toBe(true);
    const [front, rear] = crew;
    // The kart's nose is along -z.
    expect(front?.z).toBeLessThan(-40 - KART.length / 2 - 1);
    expect(rear?.z).toBeGreaterThan(-40 + KART.length / 2 + 1);
    expect(crew[6]?.z).toBeLessThan(-40 - KART.length / 2 - 1);
    // The other pit box's crew waits on.
    for (const member of crewAt(pits, 1)) expect(member?.x).toBeGreaterThan(10 + KART.width);
  });

  it("acts the roll out past a clean stop's time: one gunner stays on, four swarm a wheel, or all stand off", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    const atWheels = () => GUNNERS.filter((member) => atAWheel(crewAt(pits, 0)[member] ?? null));
    pits.show([stopping(0, 3.2, "slow", 3.8), kartReadout()], AT_THE_BOXES, 0.1);
    expect(atWheels()).toHaveLength(4);
    pits.show([stopping(0, 5, "botched", 6), kartReadout()], AT_THE_BOXES, 0.1);
    const swarm = GUNNERS.map((member) => crewAt(pits, 0)[member]);
    for (const gunner of swarm) expect(gunner?.x).toBeLessThan(10 - KART.width / 2);
    pits.show([stopping(0, 4, "disaster", 10.5), kartReadout()], AT_THE_BOXES, 0.1);
    expect(atWheels()).toHaveLength(0);
  });

  it("hangs an air line from the boom to every gun", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    pits.show([stopping(0, 1), kartReadout()], AT_THE_BOXES, 0.1);
    const lines = pits.mesh.getObjectByName("airLines");
    if (!(lines instanceof LineSegments)) throw new Error("the air lines are line segments");
    const position = lines.geometry.getAttribute("position");
    const segmentsPerHose = position.count / (BOXES.length * 4) / 2;
    for (let hose = 0; hose < 4; hose += 1) {
      const first = hose * segmentsPerHose * 2;
      expect(position.getY(first)).toBeGreaterThan(BOOM_HEIGHT_M - 0.5);
      const last = first + segmentsPerHose * 2 - 1;
      expect(position.getY(last)).toBeLessThan(1.5);
    }
  });

  it("draws no crew while the camera is out of range of the lane", () => {
    const pits = pitBoxMeshes(BOXES, KART, OPTIONS);
    pits.show([stopping(0, 1), kartReadout()], FAR_AWAY, 0.1);
    expect(crewAt(pits, 0).every((member) => member === null)).toBe(true);
    expect(partPieces(pits, "headset").every((piece) => !drawn(piece, 0))).toBe(true);
    expect(pits.mesh.getObjectByName("pitBox0")?.getObjectByName("boom")?.visible).toBe(false);
    pits.show([stopping(0, 1), kartReadout()], AT_THE_BOXES, 0.1);
    expect(crewAt(pits, 0).every((member) => member !== null)).toBe(true);
    expect(pits.mesh.getObjectByName("pitBox0")?.getObjectByName("boom")?.visible).toBe(true);
  });
});
