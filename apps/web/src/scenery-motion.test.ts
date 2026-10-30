import { Group, type Object3D, Quaternion, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import { sceneryMotion, WHEEL_TURN_MS } from "./scenery-motion";

const GONDOLAS = 16;
const RADIUS_M = 12;
const UP = new Vector3(0, 1, 0);
const AXLE = new Vector3(0, 0, 1);

/**
 * A track with the Circuit Wheel as the loader hands it over: the rim's
 * node turned to its heading, with a gondola child at each rim corner,
 * named as the export names them with the dots dropped.
 */
function trackWithWheel(heading: number): { track: Object3D; rim: Object3D; gondolas: Object3D[] } {
  const track = new Group();
  const rim = new Group();
  rim.name = "Propcircuit-wheel1rim";
  rim.position.set(80, 14, -60);
  rim.quaternion.setFromAxisAngle(UP, heading);
  track.add(rim);
  const gondolas = Array.from({ length: GONDOLAS }, (_, index) => {
    const gondola = new Group();
    gondola.name = `Propcircuit-wheel1gondola${index + 1}`;
    const corner = (Math.PI * 2 * index) / GONDOLAS;
    gondola.position.set(RADIUS_M * Math.cos(corner), RADIUS_M * Math.sin(corner), 0);
    rim.add(gondola);
    return gondola;
  });
  return { track, rim, gondolas };
}

/** The angle, in turns, the rim has turned about its axle from `rest`. */
function turnsFrom(rest: Quaternion, node: Object3D): number {
  const spin = rest.clone().invert().multiply(node.quaternion);
  const axis = new Vector3(spin.x, spin.y, spin.z);
  const angle = 2 * Math.atan2(axis.length(), spin.w);
  const sign = axis.lengthSq() === 0 ? 1 : Math.sign(axis.normalize().dot(AXLE));
  return ((((sign * angle) / (Math.PI * 2)) % 1) + 1) % 1;
}

describe("the Circuit Wheel's motion", () => {
  // A heading past a right angle, where an Euler read of the node would flip.
  const heading = 2.5;

  it("turns the rim a full turn every 90 s from where the export left it", () => {
    const { track, rim } = trackWithWheel(heading);
    const rest = rim.quaternion.clone();
    const motion = sceneryMotion(track);
    motion.update(0);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0, 6);
    motion.update(WHEEL_TURN_MS / 4);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0.25, 6);
    motion.update(WHEEL_TURN_MS / 2);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0.5, 6);
    motion.update(WHEEL_TURN_MS * 3);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0, 6);
    motion.update(WHEEL_TURN_MS * 3.1);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0.1, 6);
    // It turns about its axle, so the axle itself keeps the export's heading.
    const axle = AXLE.clone().applyQuaternion(rim.quaternion);
    expect(axle.distanceTo(AXLE.clone().applyQuaternion(rest))).toBeCloseTo(0, 6);
  });

  it("keeps every gondola level while the rim carries it round", () => {
    const { track, rim, gondolas } = trackWithWheel(heading);
    const level = rim.quaternion.clone();
    const motion = sceneryMotion(track);
    const top = gondolas[4];
    if (top === undefined) throw new Error("sixteen gondolas");
    rim.updateMatrixWorld(true);
    const before = top.getWorldPosition(new Vector3());
    for (const nowMs of [0, 1234, WHEEL_TURN_MS / 3, WHEEL_TURN_MS / 2, WHEEL_TURN_MS * 0.9]) {
      motion.update(nowMs);
      rim.updateMatrixWorld(true);
      for (const gondola of gondolas) {
        const world = gondola.getWorldQuaternion(new Quaternion());
        expect(Math.abs(world.dot(level)), `gondola ${gondola.name} at ${nowMs} ms`).toBeCloseTo(
          1,
          6,
        );
      }
    }
    // Half a turn on, the gondola that hung at the top hangs at the bottom.
    motion.update(WHEEL_TURN_MS / 2);
    rim.updateMatrixWorld(true);
    const after = top.getWorldPosition(new Vector3());
    expect(after.y).toBeCloseTo(rim.position.y - (before.y - rim.position.y), 6);
    expect(after.distanceTo(rim.position)).toBeCloseTo(RADIUS_M, 6);
  });

  it("leaves a track without a wheel alone", () => {
    const track = new Group();
    const kerb = new Group();
    kerb.name = "Kerb.kerb-red";
    track.add(kerb);
    const before = kerb.quaternion.clone();
    sceneryMotion(track).update(12_345);
    expect(kerb.quaternion.equals(before)).toBe(true);
  });
});
