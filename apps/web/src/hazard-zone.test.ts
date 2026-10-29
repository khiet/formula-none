import { describe, expect, it } from "vitest";
import type { Wheel } from "./bridge";
import { hazardFlashOpacity } from "./hazard-zone";

const QUIET: Wheel = { kind: "quiet" };
const WARNING: Wheel = { kind: "warning", position: [0, 0, 0], yaw: 0 };
const ROLLING: Wheel = {
  kind: "rolling",
  position: [0, 0, 0],
  yaw: 0,
  fraction: 0.5,
  legFraction: 0.5,
  crossingYaw: 0,
};

/** A second of the View's clock sampled every frame at 60 Hz. */
const A_SECOND = Array.from({ length: 60 }, (_, frame) => frame / 60);

describe("the hazard zone's flash", () => {
  it("is off while the wheel is quiet", () => {
    for (const clock of A_SECOND) expect(hazardFlashOpacity(QUIET, clock)).toBe(0);
  });

  it.each([
    ["the flag is up", WARNING],
    ["the wheel rolls", ROLLING],
  ])("pulses between 20%% and 60%% while %s", (_, wheel) => {
    const opacities = A_SECOND.map((clock) => hazardFlashOpacity(wheel, clock));
    for (const opacity of opacities) {
      expect(opacity).toBeGreaterThanOrEqual(0.2 - 1e-9);
      expect(opacity).toBeLessThanOrEqual(0.6 + 1e-9);
    }
    // At 14 rad/s a second holds over two pulses, reaching both ends.
    expect(Math.min(...opacities)).toBeCloseTo(0.2, 2);
    expect(Math.max(...opacities)).toBeCloseTo(0.6, 2);
    // Peaks a quarter turn in, troughs three quarters in.
    expect(hazardFlashOpacity(wheel, Math.PI / 2 / 14)).toBeCloseTo(0.6, 6);
    expect(hazardFlashOpacity(wheel, (3 * Math.PI) / 2 / 14)).toBeCloseTo(0.2, 6);
  });
});
