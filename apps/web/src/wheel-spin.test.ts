import { BoxGeometry, Group, Mesh, type Object3D, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import { kartWheels, WHEELS, wheelSpin } from "./wheel-spin";

const FRAME_S = 1 / 60;
const RADIUS_M = 0.25;

describe("a wheel's roll", () => {
  it("rolls forwards by the distance covered over its radius", () => {
    expect(wheelSpin(0, 5, RADIUS_M, FRAME_S)).toBeCloseTo((5 / RADIUS_M) * FRAME_S);
  });

  it("rolls on from where it stood", () => {
    const once = wheelSpin(0, 5, RADIUS_M, FRAME_S);
    expect(wheelSpin(once, 5, RADIUS_M, FRAME_S)).toBeCloseTo(2 * once);
  });

  it("rolls backwards while the kart backs up", () => {
    expect(wheelSpin(1, -5, RADIUS_M, FRAME_S)).toBeCloseTo(1 - (5 / RADIUS_M) * FRAME_S);
  });

  it("stands still at zero speed", () => {
    expect(wheelSpin(1, 0, RADIUS_M, FRAME_S)).toBe(1);
  });

  it("turns under half the three spokes' 120 degree repeat in a frame, so a fast wheel never reads as turning backwards", () => {
    // About 106 degrees a frame uncapped, past half the spokes' repeat.
    const forwards = wheelSpin(1, 30, 0.27, FRAME_S) - 1;
    expect(forwards).toBeGreaterThan(0);
    expect(forwards).toBeLessThan(Math.PI / 3);
    const backwards = wheelSpin(1, -30, 0.27, FRAME_S) - 1;
    expect(backwards).toBeLessThan(0);
    expect(backwards).toBeGreaterThan(-Math.PI / 3);
  });

  it("keeps the angle within a turn, so it does not grow over a long race", () => {
    let angle = 0;
    for (let frame = 0; frame < 60 * 600; frame += 1) {
      angle = wheelSpin(angle, 20, RADIUS_M, FRAME_S);
    }
    expect(Math.abs(angle)).toBeLessThan(2 * Math.PI);
  });
});

/** A stand-in kart: each wheel a 0.6 m tall block at its corner, the way the exporter places them. */
function model(): Group {
  const root = new Group();
  WHEELS.forEach((name, index) => {
    const wheel = new Group();
    wheel.name = name;
    wheel.position.set(index % 2 === 0 ? -0.5 : 0.5, 0.3, index < 2 ? -0.5 : 0.5);
    wheel.add(new Mesh(new BoxGeometry(0.2, 0.6, 0.6)));
    root.add(wheel);
  });
  return root;
}

/** Where the top of `wheel` has turned to, relative to its axle. */
function topOf(root: Object3D, name: string): Vector3 {
  const wheel = root.getObjectByName(name);
  if (wheel === undefined) throw new Error(`no ${name}`);
  return new Vector3(0, 1, 0).applyEuler(wheel.rotation);
}

describe("a kart's wheels", () => {
  it("roll every wheel by the speed over the radius measured from its bounds at the drawn scale", () => {
    const kart = model();
    // 0.3 m from the bounds, drawn twice the size: 0.6 m.
    kartWheels(kart, 2).roll(3, FRAME_S);
    for (const name of WHEELS) {
      expect(topOf(kart, name).angleTo(new Vector3(0, 1, 0))).toBeCloseTo((3 / 0.6) * FRAME_S);
    }
  });

  it("carry the top of each wheel towards the kart's front, -Z, as it drives forwards", () => {
    const kart = model();
    kartWheels(kart, 1).roll(3, FRAME_S);
    expect(topOf(kart, "wheel-back-right").z).toBeLessThan(0);
  });

  it("carry it back towards +Z as the kart backs up", () => {
    const kart = model();
    kartWheels(kart, 1).roll(-3, FRAME_S);
    expect(topOf(kart, "wheel-back-right").z).toBeGreaterThan(0);
  });
});
