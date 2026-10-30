import { describe, expect, it } from "vitest";
import { type DroneFlight, droneTilt, flyDrones } from "./homing-drones";

const DEGREE = Math.PI / 180;
const FRAME_S = 1 / 60;

/** The tilt after holding a turn of `radPerS` for `seconds`, from level, a frame at a time. */
function held(radPerS: number, seconds: number, from = 0): number {
  let tilt = from;
  let yaw = 0;
  for (let frame = 0; frame < Math.round(seconds / FRAME_S); frame += 1) {
    tilt = droneTilt(yaw, yaw + radPerS * FRAME_S, tilt, FRAME_S);
    yaw += radPerS * FRAME_S;
  }
  return tilt;
}

describe("a homing drone's tilt", () => {
  it("drops the left side in a left turn and the right side in a right turn", () => {
    // Yaw grows to the left; a positive tilt drops the left side.
    expect(held(1, 1)).toBeGreaterThan(0);
    expect(held(-1, 1)).toBeLessThan(0);
  });

  it("grows with the turn rate up to 30 degrees at 2 rad/s, and holds there beyond", () => {
    expect(held(1, 2) / DEGREE).toBeCloseTo(15, 1);
    expect(held(2, 2) / DEGREE).toBeCloseTo(30, 1);
    expect(held(5, 2) / DEGREE).toBeCloseTo(30, 1);
    expect(held(-5, 2) / DEGREE).toBeCloseTo(-30, 1);
  });

  it("eases toward the tilt over 0.12 s rather than snapping to it", () => {
    const first = droneTilt(0, 2 * FRAME_S, 0, FRAME_S);
    expect(first).toBeGreaterThan(0);
    expect(first).toBeLessThan(10 * DEGREE);
    // One time constant in, it has come about 63% of the way.
    expect(held(2, 0.12) / (30 * DEGREE)).toBeCloseTo(1 - Math.exp(-1), 1);
  });

  it("rolls back to level on a straight", () => {
    expect(held(0, 0.6, 30 * DEGREE)).toBeCloseTo(0, 2);
  });

  it("reads crossing the yaw seam as the small turn it is", () => {
    const acrossSeam = droneTilt(Math.PI - 0.01, -Math.PI + 0.01, 0, FRAME_S);
    const sameTurn = droneTilt(0, 0.02, 0, FRAME_S);
    expect(acrossSeam).toBeCloseTo(sameTurn, 9);
  });

  it("starts level on a newly fired drone, which has no facing to turn from", () => {
    expect(droneTilt(undefined, 1, 0, FRAME_S)).toBe(0);
  });

  it("holds its tilt through a frame that shows no time", () => {
    expect(droneTilt(0, 0.5, 0.2, 0)).toBe(0.2);
  });
});

describe("the homing drones from frame to frame", () => {
  const flying = (x: number, yaw: number, tilt: number): DroneFlight => ({
    position: [x, 0, 0],
    yaw,
    tilt,
  });

  it("keeps each drone's own tilt when an earlier one hits and the rest move up a place", () => {
    const before = [flying(0, 0, 0.1), flying(50, 0, 0.2), flying(100, 0, 0.3)];
    const after = flyDrones(
      before,
      [
        { position: [50.5, 0, 0], yaw: 0 },
        { position: [100.5, 0, 0], yaw: 0 },
      ],
      FRAME_S,
    );
    expect(after.map(({ tilt }) => tilt)).toEqual([
      droneTilt(0, 0, 0.2, FRAME_S),
      droneTilt(0, 0, 0.3, FRAME_S),
    ]);
  });

  it("turns each drone from its own last facing", () => {
    const after = flyDrones(
      [flying(0, 0, 0), flying(50, 1, 0)],
      [
        { position: [0.5, 0, 0], yaw: 0 },
        { position: [50.5, 0, 0], yaw: 1.02 },
      ],
      FRAME_S,
    );
    expect(after[0]?.tilt).toBe(0);
    expect(after[1]?.tilt).toBeGreaterThan(0);
  });

  it("holds a drone with no kart to face on its own last facing, not its old neighbour's", () => {
    const after = flyDrones(
      [flying(0, 0, 0), flying(50, 2, 0.3)],
      [{ position: [50.5, 0, 0], yaw: undefined }],
      FRAME_S,
    );
    expect(after).toEqual([
      { position: [50.5, 0, 0], yaw: 2, tilt: droneTilt(2, 2, 0.3, FRAME_S) },
    ]);
  });

  it("starts a newly fired drone level, even beside one already tilted", () => {
    const after = flyDrones(
      [flying(0, 0, 0.4)],
      [
        { position: [0.5, 0, 0], yaw: 0.02 },
        { position: [80, 0, 0], yaw: 3 },
      ],
      FRAME_S,
    );
    expect(after[0]?.tilt).toBeGreaterThan(0.3);
    expect(after[1]).toEqual({ position: [80, 0, 0], yaw: 3, tilt: 0 });
  });
});
