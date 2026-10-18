import { describe, expect, it } from "vitest";
import type { KartInput } from "./bridge";
import { combinedKartInput, type KartInputSource } from "./input";

const IDLE: KartInput = {
  steer: 0,
  throttle: 0,
  brake: 0,
  hop: false,
  useItem: false,
  holdItem: false,
};

function source(input: Partial<KartInput>): KartInputSource {
  return { current: () => ({ ...IDLE, ...input }) };
}

describe("keyboard and gamepad combine into one input", () => {
  it("is idle when every device is", () => {
    expect(combinedKartInput(source({}), source({})).current()).toEqual(IDLE);
  });

  it("takes each axis from the device pushing it hardest, so swapping devices needs no setting", () => {
    const keyboard = source({ throttle: 1, steer: -1 });
    const gamepad = source({ throttle: 0.4, steer: 0.2, brake: 0.3 });
    expect(combinedKartInput(keyboard, gamepad).current()).toEqual({
      ...IDLE,
      steer: -1,
      throttle: 1,
      brake: 0.3,
    });
    expect(combinedKartInput(source({ steer: 0.5 }), source({ steer: -0.9 })).current().steer).toBe(
      -0.9,
    );
  });

  it("a button pressed on any device is pressed", () => {
    const combined = combinedKartInput(source({ hop: true }), source({ useItem: true }));
    expect(combined.current()).toMatchObject({ hop: true, useItem: true, holdItem: false });
  });
});
