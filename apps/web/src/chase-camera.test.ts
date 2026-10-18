import { describe, expect, it } from "vitest";
import type { KartTransform } from "./bridge";
import { createChaseCamera } from "./chase-camera";

const FRAME_S = 1 / 60;

/** A kart at `position` turned `yaw` radians about +Y from its -Z facing. */
function kart(yaw: number, position: readonly [number, number, number]): KartTransform {
  return Object.freeze({
    position: Object.freeze([...position]) as readonly [number, number, number],
    rotation: Object.freeze([0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)]) as readonly [
      number,
      number,
      number,
      number,
    ],
    speed: 0,
    surface: "asphalt",
  });
}

function expectClose(actual: readonly number[], expected: readonly number[], digits = 3) {
  expect(actual).toHaveLength(expected.length);
  actual.forEach((value, index) => {
    expect(value, `component ${index}`).toBeCloseTo(expected[index] ?? Number.NaN, digits);
  });
}

/** The camera's yaw about the kart, read back from where it sits. */
function yawOf(position: readonly number[], around: readonly number[]): number {
  return Math.atan2((position[0] ?? 0) - (around[0] ?? 0), (position[2] ?? 0) - (around[2] ?? 0));
}

describe("the chase camera", () => {
  it("sits 6 m behind and 2.5 m above the kart and looks 4 m ahead of it", () => {
    const camera = createChaseCamera();
    const pose = camera.update(kart(0, [10, 0.3, -20]), FRAME_S);
    expectClose(pose.position, [10, 2.8, -14]);
    expectClose(pose.target, [10, 0.3, -24]);
  });

  it("lags a kart that turns and settles behind it again without swinging past", () => {
    const camera = createChaseCamera();
    const facingMinusZ = kart(0, [0, 0.3, 0]);
    camera.update(facingMinusZ, FRAME_S);
    // A quarter turn to the right in one tick: the camera cannot keep up.
    const facingPlusX = kart(-Math.PI / 2, [0, 0.3, 0]);
    const first = camera.update(facingPlusX, FRAME_S);
    const lag = yawOf(first.position, facingPlusX.position);
    expect(lag).toBeLessThan(-0.001);
    expect(lag).toBeGreaterThan(-Math.PI / 2 + 0.3);

    let previous = lag;
    for (let frame = 0; frame < 3 * 60; frame++) {
      const pose = camera.update(facingPlusX, FRAME_S);
      const yaw = yawOf(pose.position, facingPlusX.position);
      expect(yaw).toBeLessThanOrEqual(previous + 1e-6);
      expect(yaw).toBeGreaterThanOrEqual(-Math.PI / 2 - 1e-6);
      previous = yaw;
    }
    const settled = camera.update(facingPlusX, FRAME_S);
    expectClose(settled.position, [-6, 2.8, 0], 2);
  });

  it("looks ahead along the kart's own heading while it still lags", () => {
    const camera = createChaseCamera();
    camera.update(kart(0, [0, 0.3, 0]), FRAME_S);
    const pose = camera.update(kart(-Math.PI / 2, [0, 0.3, 0]), FRAME_S);
    expectClose(pose.target, [4, 0.3, 0]);
  });

  it("takes the shortest way round when the kart's yaw wraps", () => {
    const camera = createChaseCamera();
    camera.update(kart(Math.PI - 0.1, [0, 0.3, 0]), FRAME_S);
    const pose = camera.update(kart(-Math.PI + 0.1, [0, 0.3, 0]), FRAME_S);
    // Just past -Z the long way is nearly a full circle; the camera must stay near +Z.
    expect(Math.abs(yawOf(pose.position, [0, 0.3, 0]))).toBeGreaterThan(Math.PI - 0.2);
  });

  it("survives a long frame", () => {
    const camera = createChaseCamera();
    camera.update(kart(0, [0, 0.3, 0]), FRAME_S);
    const pose = camera.update(kart(-Math.PI / 2, [0, 0.3, 0]), 2);
    expect(pose.position.every(Number.isFinite)).toBe(true);
    expectClose(pose.position, [-6, 2.8, 0], 1);
  });

  it("only reads the transforms it is given, so it cannot reach the sim", () => {
    const camera = createChaseCamera();
    const frozen = kart(0.4, [1, 0.3, 2]);
    expect(() => camera.update(frozen, FRAME_S)).not.toThrow();
    expect(frozen).toEqual(kart(0.4, [1, 0.3, 2]));
  });
});
