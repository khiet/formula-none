import { describe, expect, it } from "vitest";
import type { Drift, KartSize, KartTransform } from "./bridge";
import { createTyreMarks, type TyreMark } from "./tyre-marks";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };
const FRAME_S = 1 / 60;
const DRIFT: Drift = { direction: "left", chargeSeconds: 0.2, miniTurbo: "none" };

/** A kart at `position` turned `yaw` about +Y from its -Z facing, drifting or not. */
function kart(
  position: readonly [number, number, number],
  drift: Drift | null,
  yaw = 0,
): KartTransform {
  return {
    position,
    rotation: [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)],
    speed: 20,
    surface: "asphalt",
    drift,
    heldItem: null,
    spinOutSeconds: null,
  };
}

function marksOf(marks: ReturnType<typeof createTyreMarks>): TyreMark[] {
  const out: TyreMark[] = [];
  marks.forEach((mark) => {
    out.push({ ...mark, from: [...mark.from], to: [...mark.to] });
  });
  return out;
}

describe("tyre marks", () => {
  it("lays nothing under a kart that is not drifting", () => {
    const marks = createTyreMarks(1, SIZE);
    marks.update([kart([0, 0.3, 0], null)], FRAME_S);
    marks.update([kart([0, 0.3, -0.5], null)], FRAME_S);
    expect(marksOf(marks)).toEqual([]);
  });

  it("lays a mark on the ground under each rear wheel between two frames of a drift", () => {
    const marks = createTyreMarks(1, SIZE);
    marks.update([kart([0, 0.3, 0], DRIFT)], FRAME_S);
    expect(marksOf(marks)).toEqual([]);
    marks.update([kart([0, 0.3, -0.5], DRIFT)], FRAME_S);
    const laid = marksOf(marks);
    expect(laid).toHaveLength(2);
    expect(laid.map((mark) => Math.sign(mark.from[0])).sort()).toEqual([-1, 1]);
    for (const mark of laid) {
      // Under the tyres: within the kart's width, on the floor of its box, behind its centre.
      expect(Math.abs(mark.from[0])).toBeLessThan(SIZE.width / 2);
      expect(Math.abs(mark.from[0])).toBeGreaterThan(SIZE.width / 4);
      expect(mark.from[1]).toBeCloseTo(0);
      expect(mark.to[1]).toBeCloseTo(0);
      expect(mark.from[2]).toBeGreaterThan(0);
      expect(mark.from[2]).toBeLessThan(SIZE.length / 2);
      expect(mark.to[2]).toBeCloseTo(mark.from[2] - 0.5);
      expect(mark.to[0]).toBeCloseTo(mark.from[0]);
      expect(mark.strength).toBeCloseTo(1, 1);
    }
  });

  it("lays the marks where the kart's wheels are when it is turned", () => {
    const marks = createTyreMarks(1, SIZE);
    // Facing +X: the rear is -X and the right wheel is at +Z.
    marks.update([kart([0, 0.3, 0], DRIFT, -Math.PI / 2)], FRAME_S);
    marks.update([kart([0.5, 0.3, 0], DRIFT, -Math.PI / 2)], FRAME_S);
    const laid = marksOf(marks);
    expect(laid).toHaveLength(2);
    expect(laid.map((mark) => Math.sign(mark.from[2])).sort()).toEqual([-1, 1]);
    for (const mark of laid) {
      expect(mark.from[0]).toBeLessThan(0);
      expect(mark.to[0]).toBeCloseTo(mark.from[0] + 0.5);
    }
  });

  it("fades a mark over its lifetime and drops it after", () => {
    const marks = createTyreMarks(1, SIZE, { lifetimeSeconds: 2 });
    marks.update([kart([0, 0.3, 0], DRIFT)], FRAME_S);
    marks.update([kart([0, 0.3, -0.5], DRIFT)], FRAME_S);
    const fresh = marksOf(marks)[0]?.strength ?? Number.NaN;
    marks.update([kart([0, 0.3, -0.5], null)], 1);
    const older = marksOf(marks)[0]?.strength ?? Number.NaN;
    expect(older).toBeLessThan(fresh);
    expect(older).toBeGreaterThan(0);
    marks.update([kart([0, 0.3, -0.5], null)], 1);
    expect(marksOf(marks)).toEqual([]);
  });

  it("breaks the trail when the drift ends, so the next drift's marks start afresh", () => {
    const marks = createTyreMarks(1, SIZE);
    marks.update([kart([0, 0.3, 0], DRIFT)], FRAME_S);
    marks.update([kart([0, 0.3, -0.5], DRIFT)], FRAME_S);
    marks.update([kart([0, 0.3, -1], null)], FRAME_S);
    marks.update([kart([0, 0.3, -1.5], DRIFT)], FRAME_S);
    marks.update([kart([0, 0.3, -2], DRIFT)], FRAME_S);
    const spans = marksOf(marks)
      .map((mark) => [mark.from[2], mark.to[2]].map((z) => Math.round(z * 10) / 10))
      .map(([from, to]) => (to ?? 0) - (from ?? 0));
    expect(spans).toHaveLength(4);
    expect(spans.every((span) => Math.abs(span + 0.5) < 1e-6)).toBe(true);
  });

  it("lays nothing for a drifting kart that has not moved", () => {
    const marks = createTyreMarks(1, SIZE);
    marks.update([kart([0, 0.3, 0], DRIFT)], FRAME_S);
    marks.update([kart([0, 0.3, 0], DRIFT)], FRAME_S);
    expect(marksOf(marks)).toEqual([]);
  });

  it("replaces the oldest marks when it is full", () => {
    const marks = createTyreMarks(1, SIZE, { capacityPerWheel: 2 });
    marks.update([kart([0, 0.3, 0], DRIFT)], FRAME_S);
    marks.update([kart([0, 0.3, -0.5], DRIFT)], FRAME_S);
    const rearmost = Math.max(...marksOf(marks).map((mark) => mark.from[2]));
    marks.update([kart([0, 0.3, -1], DRIFT)], FRAME_S);
    marks.update([kart([0, 0.3, -1.5], DRIFT)], FRAME_S);
    const laid = marksOf(marks);
    expect(laid).toHaveLength(4);
    // The first frame's pair has made room for the newest.
    expect(Math.max(...laid.map((mark) => mark.from[2]))).toBeLessThan(rearmost - 0.4);
    expect(Math.min(...laid.map((mark) => mark.to[2]))).toBeCloseTo(rearmost - 1.5);
  });

  it("marks every kart in a grid of drifters", () => {
    const marks = createTyreMarks(8, SIZE);
    const grid = (z: number) =>
      Array.from({ length: 8 }, (_, slot) => kart([slot * 3, 0.3, z], DRIFT));
    marks.update(grid(0), FRAME_S);
    marks.update(grid(-0.5), FRAME_S);
    expect(marksOf(marks)).toHaveLength(16);
  });
});
