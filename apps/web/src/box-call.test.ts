import { describe, expect, it } from "vitest";
import { BOX_CALL_S, createBoxCall } from "./box-call";

const RULES = { pitLane: true, lapsPerRace: 3, cliff: 30 };
const FRAME_S = 1 / 60;

describe("the BOX call", () => {
  it("stays down above the cliff and comes up as life first falls to it", () => {
    const call = createBoxCall(RULES);
    expect(call.update(30.5, 1, FRAME_S)).toBe(false);
    expect(call.update(30, 1, FRAME_S)).toBe(true);
  });

  it("shows for its two seconds and comes down", () => {
    const call = createBoxCall(RULES);
    expect(call.update(29, 1, 0)).toBe(true);
    expect(call.update(28, 1, BOX_CALL_S - 0.1)).toBe(true);
    expect(call.update(27, 1, 0.2)).toBe(false);
  });

  it("is not repeated when a correction takes life back over the cliff", () => {
    const call = createBoxCall(RULES);
    call.update(29.9, 1, 0);
    call.update(30.4, 1, BOX_CALL_S + 1);
    expect(call.update(29.9, 1, FRAME_S)).toBe(false);
  });

  it("does not come up on a course without a pit lane", () => {
    const call = createBoxCall({ ...RULES, pitLane: false });
    expect(call.update(10, 1, FRAME_S)).toBe(false);
  });

  it("does not come up when life reaches the cliff on the final lap", () => {
    const call = createBoxCall(RULES);
    expect(call.update(29, 3, FRAME_S)).toBe(false);
    expect(call.update(28, 3, FRAME_S)).toBe(false);
  });
});
