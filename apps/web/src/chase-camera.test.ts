import { describe, expect, it } from "vitest";
import type { CameraTuning, KartTransform } from "./bridge";
import { createChaseCamera } from "./chase-camera";

const FRAME_S = 1 / 60;
/** A fixture of tuned values; in the app the camera reads the live ones from the bridge. */
const TUNING: CameraTuning = {
  topSpeedMetresPerSecond: 30,
  pullBackMetres: 1,
  shakeSeconds: 0.3,
  shakeAmplitudeMetres: 0.15,
};

/** A kart at `position` turned `yaw` radians about +Y from its -Z facing. */
function kart(
  yaw: number,
  position: readonly [number, number, number],
  rest: Partial<KartTransform> = {},
): KartTransform {
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
    drift: null,
    heldItem: null,
    spinOutSeconds: null,
    airborne: false,
    boostSeconds: null,
    start: "clean",
    stallSeconds: null,
    ...rest,
  });
}

function distance(a: readonly number[], b: readonly number[]): number {
  return Math.hypot(...a.map((value, index) => value - (b[index] ?? 0)));
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
  it("sits 6 m behind and 3.2 m above the kart and looks 4 m ahead of it", () => {
    const camera = createChaseCamera(TUNING);
    const pose = camera.update(kart(0, [10, 0.3, -20]), FRAME_S);
    expectClose(pose.position, [10, 3.5, -14]);
    expectClose(pose.target, [10, 0.3, -24]);
  });

  it("lags a kart that turns and settles behind it again without swinging past", () => {
    const camera = createChaseCamera(TUNING);
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
    expectClose(settled.position, [-6, 3.5, 0], 2);
  });

  it("looks ahead along the kart's own heading while it still lags", () => {
    const camera = createChaseCamera(TUNING);
    camera.update(kart(0, [0, 0.3, 0]), FRAME_S);
    const pose = camera.update(kart(-Math.PI / 2, [0, 0.3, 0]), FRAME_S);
    expectClose(pose.target, [4, 0.3, 0]);
  });

  it("takes the shortest way round when the kart's yaw wraps", () => {
    const camera = createChaseCamera(TUNING);
    camera.update(kart(Math.PI - 0.1, [0, 0.3, 0]), FRAME_S);
    const pose = camera.update(kart(-Math.PI + 0.1, [0, 0.3, 0]), FRAME_S);
    // Just past -Z the long way is nearly a full circle; the camera must stay near +Z.
    expect(Math.abs(yawOf(pose.position, [0, 0.3, 0]))).toBeGreaterThan(Math.PI - 0.2);
  });

  it("survives a long frame", () => {
    const camera = createChaseCamera(TUNING);
    camera.update(kart(0, [0, 0.3, 0]), FRAME_S);
    const pose = camera.update(kart(-Math.PI / 2, [0, 0.3, 0]), 2);
    expect(pose.position.every(Number.isFinite)).toBe(true);
    expectClose(pose.position, [-6, 3.5, 0], 1);
  });

  it("pulls back 1 m at top speed, scaling with speed, and further on a boost", () => {
    const at = (speed: number) =>
      createChaseCamera(TUNING).update(kart(0, [0, 0.3, 0], { speed }), FRAME_S);
    expectClose(at(0).position, [0, 3.5, 6]);
    expectClose(at(15).position, [0, 3.5, 6.5]);
    expectClose(at(30).position, [0, 3.5, 7]);
    expectClose(at(42).position, [0, 3.5, 7.4]);
    // Reversing is slow and the camera stays put.
    expectClose(at(-8).position, [0, 3.5, 6]);
    // The look-ahead is the kart's, not the camera's.
    expectClose(at(30).target, [0, 0.3, -4]);
  });

  it("shakes for the tuned 0.3 s when the kart is hit and settles back on its pose", () => {
    const camera = createChaseCamera(TUNING);
    const still = kart(0, [0, 0.3, 0]);
    const rest = camera.update(still, FRAME_S);
    let shakenFrames = 0;
    let furthest = 0;
    // The hit: the sim reports the spin-out starting and counting down.
    for (let frame = 0; frame < 18; frame++) {
      const spun = kart(0, [0, 0.3, 0], { spinOutSeconds: 1 - frame * FRAME_S });
      const pose = camera.update(spun, FRAME_S);
      const thrown = distance(pose.position, rest.position);
      expect(distance(pose.target, rest.target)).toBeCloseTo(thrown, 6);
      furthest = Math.max(furthest, thrown);
      if (thrown > 1e-3) shakenFrames += 1;
    }
    expect(shakenFrames).toBeGreaterThanOrEqual(15);
    expect(furthest).toBeGreaterThan(TUNING.shakeAmplitudeMetres / 4);
    expect(furthest).toBeLessThanOrEqual(TUNING.shakeAmplitudeMetres * Math.sqrt(3) + 1e-9);
    const settled = camera.update(kart(0, [0, 0.3, 0], { spinOutSeconds: 0.6 }), FRAME_S);
    expectClose(settled.position, rest.position, 9);
    expectClose(settled.target, rest.target, 9);
  });

  it("shakes again for a second hit once the first spin-out has ended", () => {
    const camera = createChaseCamera(TUNING);
    const still = kart(0, [0, 0.3, 0]);
    const rest = camera.update(still, FRAME_S);
    camera.update(kart(0, [0, 0.3, 0], { spinOutSeconds: 1 }), FRAME_S);
    // The first spin-out runs out and the shake with it.
    for (let frame = 0; frame < 60; frame++) {
      camera.update(still, FRAME_S);
    }
    expectClose(camera.update(still, FRAME_S).position, rest.position, 9);
    const hitAgain = camera.update(kart(0, [0, 0.3, 0], { spinOutSeconds: 1 }), FRAME_S);
    expect(distance(hitAgain.position, rest.position)).toBeGreaterThan(1e-3);
  });

  it("does not shake for a kart already spun out when first seen, nor through the spin", () => {
    const camera = createChaseCamera(TUNING);
    const spinning = (left: number) => kart(0, [0, 0.3, 0], { spinOutSeconds: left });
    const first = camera.update(spinning(0.9), FRAME_S);
    expectClose(first.position, [0, 3.5, 6], 9);
    for (let frame = 1; frame < 30; frame++) {
      expectClose(camera.update(spinning(0.9 - frame * FRAME_S), FRAME_S).position, [0, 3.5, 6], 9);
    }
  });

  it("follows a hop by a bounded fraction of the kart's rise and settles back", () => {
    const camera = createChaseCamera(TUNING);
    const rest = camera.update(kart(0, [0, 0.3, 0]), FRAME_S);
    // A model of the sim's hop: 0.3 s of airtime under its gravity, peaking
    // about 11 cm up. The test owns these figures; a retuned hop in the sim
    // would not change what the camera must do with one.
    const airtime = 0.3;
    const gravity = 9.81;
    const launch = (gravity * airtime) / 2;
    const peak = (launch * launch) / (2 * gravity);
    let cameraRise = 0;
    let kartRiseInFrame = 0;
    for (let frame = 1; frame * FRAME_S <= airtime; frame++) {
      const t = frame * FRAME_S;
      const y = 0.3 + launch * t - (gravity / 2) * t * t;
      const pose = camera.update(kart(0, [0, y, 0], { airborne: true }), FRAME_S);
      cameraRise = Math.max(cameraRise, pose.position[1] - rest.position[1]);
      kartRiseInFrame = Math.max(kartRiseInFrame, y - pose.target[1]);
      // The look target rises with the camera, so the kart moves in the frame.
      expect(pose.position[1] - pose.target[1]).toBeCloseTo(3.2, 9);
    }
    expect(cameraRise).toBeGreaterThan(0);
    expect(cameraRise).toBeLessThan(peak / 3);
    expect(kartRiseInFrame).toBeGreaterThan((peak * 2) / 3);
    for (let frame = 0; frame < 2 * 60; frame++) {
      camera.update(kart(0, [0, 0.3, 0]), FRAME_S);
    }
    expectClose(camera.update(kart(0, [0, 0.3, 0]), FRAME_S).position, rest.position, 2);
  });

  it("follows a slope without falling far behind and settles at the new height", () => {
    const camera = createChaseCamera(TUNING);
    camera.update(kart(0, [0, 0.3, 0]), FRAME_S);
    // Brands Patch's climb: 2 m of elevation, here over 3 s at speed.
    let previousY = Number.NEGATIVE_INFINITY;
    for (let frame = 1; frame <= 3 * 60; frame++) {
      const y = 0.3 + (2 * frame) / (3 * 60);
      const pose = camera.update(kart(0, [0, y, 0]), FRAME_S);
      expect(pose.position[1]).toBeGreaterThanOrEqual(previousY);
      expect(pose.position[1]).toBeLessThanOrEqual(y + 3.2);
      expect(pose.position[1]).toBeGreaterThan(y + 3.2 - 0.5);
      previousY = pose.position[1];
    }
    for (let frame = 0; frame < 2 * 60; frame++) {
      camera.update(kart(0, [0, 2.3, 0]), FRAME_S);
    }
    const settled = camera.update(kart(0, [0, 2.3, 0]), FRAME_S);
    expectClose(settled.position, [0, 5.5, 6], 2);
    expectClose(settled.target, [0, 2.3, -4], 2);
  });

  it("leaves the transform it reads untouched", () => {
    const camera = createChaseCamera(TUNING);
    const frozen = kart(0.4, [1, 0.3, 2]);
    expect(() => camera.update(frozen, FRAME_S)).not.toThrow();
    expect(frozen).toEqual(kart(0.4, [1, 0.3, 2]));
  });
});
