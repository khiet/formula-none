import { describe, expect, it } from "vitest";
import { BOX_CALL_FROM_M, BOX_CALL_UNTIL_M, type BoxCallKart, createBoxCall } from "./box-call";

const SPLINE_LENGTH = 1000;
const FORK = 600;
const RULES = { fork: { splineProgress: FORK, splineLength: SPLINE_LENGTH }, lapsPerRace: 3 };
const TUNING = { tyreLifeCliffPercent: 30 };
const WORN = 25;

/** The own kart `metres` short of the fork on `lap`, out of the pit lane. */
function short(metres: number, overrides: Partial<BoxCallKart> = {}): BoxCallKart {
  return { tyreLife: WORN, lap: 1, splineProgress: FORK - metres, inPitLane: false, ...overrides };
}

describe("the BOX call", () => {
  it("below the cliff, opens 120 m short of the fork and closes 15 m short of it", () => {
    const call = createBoxCall(RULES, TUNING);
    expect(call.update(short(BOX_CALL_FROM_M + 1))).toBe(false);
    expect(call.update(short(BOX_CALL_FROM_M))).toBe(true);
    expect(call.update(short(60))).toBe(true);
    expect(call.update(short(BOX_CALL_UNTIL_M + 0.5))).toBe(true);
    expect(call.update(short(BOX_CALL_UNTIL_M))).toBe(false);
  });

  it("closes when the kart enters the pit lane", () => {
    const call = createBoxCall(RULES, TUNING);
    expect(call.update(short(80))).toBe(true);
    expect(call.update(short(40, { inPitLane: true }))).toBe(false);
    expect(call.update(short(30))).toBe(false);
  });

  it("stays down above the cliff", () => {
    const call = createBoxCall(RULES, TUNING);
    for (const metres of [120, 80, 40, 20]) {
      expect(call.update(short(metres, { tyreLife: 30.5 }))).toBe(false);
    }
  });

  it("opens for the rest of the window when life falls to the cliff inside it", () => {
    const call = createBoxCall(RULES, TUNING);
    expect(call.update(short(80, { tyreLife: 31 }))).toBe(false);
    expect(call.update(short(60, { tyreLife: 30 }))).toBe(true);
    expect(call.update(short(20))).toBe(true);
    expect(call.update(short(10))).toBe(false);
  });

  it("waits for the next lap when life falls to the cliff past the window", () => {
    const call = createBoxCall(RULES, TUNING);
    expect(call.update(short(20, { tyreLife: 31 }))).toBe(false);
    expect(call.update(short(10, { tyreLife: 30 }))).toBe(false);
    expect(call.update(short(-50, { tyreLife: 29 }))).toBe(false);
    expect(call.update(short(100, { lap: 2 }))).toBe(true);
  });

  it("opens again the next lap the kart stays out worn", () => {
    const call = createBoxCall(RULES, TUNING);
    expect(call.update(short(100))).toBe(true);
    expect(call.update(short(10))).toBe(false);
    expect(call.update(short(100, { lap: 2 }))).toBe(true);
  });

  it("does not reopen when a correction moves the kart back across 120 m on the same lap", () => {
    const call = createBoxCall(RULES, TUNING);
    expect(call.update(short(119))).toBe(true);
    expect(call.update(short(121))).toBe(false);
    expect(call.update(short(119))).toBe(false);
    expect(call.update(short(60))).toBe(false);
  });

  it("does not come up on the final lap", () => {
    const call = createBoxCall(RULES, TUNING);
    expect(call.update(short(100, { lap: 3 }))).toBe(false);
    expect(call.update(short(60, { lap: 3 }))).toBe(false);
  });

  it("does not come up on a course without a pit lane", () => {
    const call = createBoxCall({ ...RULES, fork: undefined }, TUNING);
    expect(call.update(short(100, { tyreLife: 5 }))).toBe(false);
  });

  it("measures to a fork just past the line from a kart just before it, once for that fork", () => {
    const call = createBoxCall(
      { fork: { splineProgress: 50, splineLength: SPLINE_LENGTH }, lapsPerRace: 3 },
      TUNING,
    );
    const at = (splineProgress: number, lap: number) =>
      call.update({ tyreLife: WORN, lap, splineProgress, inPitLane: false });
    expect(at(SPLINE_LENGTH - 80, 1)).toBe(false);
    expect(at(SPLINE_LENGTH - 60, 1)).toBe(true);
    // Over the line the lap counts on, and the fork's call carries on.
    expect(at(10, 2)).toBe(true);
    expect(at(SPLINE_LENGTH - 60, 1)).toBe(true);
    expect(at(10, 2)).toBe(true);
    expect(at(40, 2)).toBe(false);
  });

  it("does not call for a fork just past the line that the kart reaches on the final lap", () => {
    const call = createBoxCall(
      { fork: { splineProgress: 50, splineLength: SPLINE_LENGTH }, lapsPerRace: 3 },
      TUNING,
    );
    expect(
      call.update({ tyreLife: WORN, lap: 2, splineProgress: SPLINE_LENGTH - 60, inPitLane: false }),
    ).toBe(false);
  });
});
