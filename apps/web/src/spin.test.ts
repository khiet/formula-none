import { describe, expect, it } from "vitest";
import { spinOutAngle } from "./spin";

describe("spin-out tell", () => {
  it("turns the kart whole turns over the spin-out, so it ends facing as the sim has it", () => {
    expect(spinOutAngle(1, 1)).toBe(0);
    const halfway = spinOutAngle(0.5, 1);
    expect(halfway).toBeGreaterThan(0);
    expect(halfway % Math.PI).toBeCloseTo(0, 10);
    expect(spinOutAngle(0, 1) % (2 * Math.PI)).toBeCloseTo(0, 10);
    expect(spinOutAngle(0, 1)).toBeGreaterThan(halfway);
  });

  it("holds at the ends for a timer outside the spin-out", () => {
    expect(spinOutAngle(1.5, 1)).toBe(0);
    expect(spinOutAngle(-0.1, 1)).toBe(spinOutAngle(0, 1));
  });
});
