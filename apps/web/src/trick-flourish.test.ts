import { describe, expect, it } from "vitest";
import { FLOURISH_S, flourishAfter, flourishTurn, NO_FLOURISH } from "./trick-flourish";

describe("the driver's flourish", () => {
  it("sits level until a trick is earned", () => {
    const flourish = flourishAfter(NO_FLOURISH, false, 1 / 60);
    expect(flourishTurn(flourish)).toBe(0);
  });

  it("turns the head onward through the flight, not yet whole, from the frame it is earned", () => {
    const earned = flourishAfter(NO_FLOURISH, true, 1 / 60);
    expect(flourishTurn(earned)).toBe(0);
    const partway = flourishAfter(earned, true, FLOURISH_S / 2);
    expect(flourishTurn(partway)).toBeGreaterThan(0);
    expect(flourishTurn(partway)).toBeLessThan(2 * Math.PI);
  });

  it("runs out level by the clock, even if the kart lands first", () => {
    const earned = flourishAfter(NO_FLOURISH, true, 1 / 60);
    const landed = flourishAfter(earned, false, FLOURISH_S / 2);
    expect(flourishTurn(landed)).toBeGreaterThan(0);
    expect(flourishTurn(flourishAfter(landed, false, FLOURISH_S))).toBe(0);
  });

  it("runs once for a trick and again for the next", () => {
    const earned = flourishAfter(NO_FLOURISH, true, 1 / 60);
    const done = flourishAfter(earned, true, FLOURISH_S);
    expect(done.seconds).toBeNull();
    expect(flourishAfter(done, true, 1 / 60).seconds).toBeNull();
    const landed = flourishAfter(done, false, 1 / 60);
    expect(flourishAfter(landed, true, 1 / 60).seconds).toBe(0);
  });
});
