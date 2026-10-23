import { describe, expect, it } from "vitest";
import { spinOutAngle } from "./spin";

const TURN = 2 * Math.PI;

describe("spin-out tell", () => {
  it("turns the kart whole turns over the spin-out, so it ends facing as the sim has it", () => {
    expect(spinOutAngle(1, 1)).toBe(0);
    expect(spinOutAngle(0, 1) % TURN).toBeCloseTo(0, 10);
    expect(spinOutAngle(0, 1)).toBeGreaterThanOrEqual(TURN);
  });

  it("spins fast off the hit and bleeds the spin away toward the end, never turning back", () => {
    const firstHalf = spinOutAngle(0.5, 1);
    const secondHalf = spinOutAngle(0, 1) - firstHalf;
    expect(firstHalf).toBeGreaterThan(secondHalf);
    let previous = 0;
    for (let step = 1; step <= 100; step++) {
      const angle = spinOutAngle(1 - step / 100, 1);
      expect(angle).toBeGreaterThan(previous);
      previous = angle;
    }
    // The last hundredth of the spin turns less than the first did.
    expect(spinOutAngle(0, 1) - spinOutAngle(0.01, 1)).toBeLessThan(spinOutAngle(0.99, 1));
  });

  it("holds at the ends for a timer outside the spin-out", () => {
    expect(spinOutAngle(1.5, 1)).toBe(0);
    expect(spinOutAngle(-0.1, 1)).toBe(spinOutAngle(0, 1));
  });
});
