import { describe, expect, it } from "vitest";
import { type GamepadLike, gamepadKartInput } from "./gamepad";
import { IDLE_INPUT } from "./input";

/**
 * A standard-mapping gamepad as the Gamepad API reports it: left stick X on
 * axis 0, triggers on buttons 6 and 7, face buttons 0 to 3.
 */
function pad(state: {
  stickX?: number;
  leftTrigger?: number;
  rightTrigger?: number;
  pressed?: readonly number[];
  connected?: boolean;
}): GamepadLike {
  const buttons = Array.from({ length: 17 }, (_, index) => {
    const pressed = state.pressed?.includes(index) ?? false;
    return { pressed, value: pressed ? 1 : 0 };
  });
  buttons[6] = { pressed: (state.leftTrigger ?? 0) > 0, value: state.leftTrigger ?? 0 };
  buttons[7] = { pressed: (state.rightTrigger ?? 0) > 0, value: state.rightTrigger ?? 0 };
  return { connected: state.connected ?? true, axes: [state.stickX ?? 0, 0, 0, 0], buttons };
}

/** The slice of `navigator` the mapping polls, with the pads swapped per read. */
function fakeNavigator(...pads: (GamepadLike | null)[]) {
  return {
    pads,
    getGamepads() {
      return this.pads;
    },
  };
}

describe("a standard gamepad maps to one kart input", () => {
  it("is idle without a gamepad", () => {
    expect(gamepadKartInput(fakeNavigator()).current()).toEqual(IDLE_INPUT);
    expect(gamepadKartInput(fakeNavigator(null, null)).current()).toEqual(IDLE_INPUT);
  });

  it("right trigger is throttle, left trigger is brake, the left stick steers", () => {
    const navigator = fakeNavigator(pad({ stickX: 1, rightTrigger: 0.5, leftTrigger: 0.25 }));
    expect(gamepadKartInput(navigator).current()).toEqual({
      ...IDLE_INPUT,
      steer: 1,
      throttle: 0.5,
      brake: 0.25,
    });
  });

  it("ignores stick drift inside the dead zone and still reaches full lock", () => {
    const navigator = fakeNavigator(pad({ stickX: 0.05 }));
    const input = gamepadKartInput(navigator);
    expect(input.current().steer).toBe(0);
    navigator.pads = [pad({ stickX: -1 })];
    expect(input.current().steer).toBe(-1);
    navigator.pads = [pad({ stickX: -0.6 })];
    const partial = input.current().steer;
    expect(partial).toBeLessThan(-0.4);
    expect(partial).toBeGreaterThan(-0.6);
  });

  it("the bottom face button hops while held", () => {
    const navigator = fakeNavigator(pad({ pressed: [0] }));
    const input = gamepadKartInput(navigator);
    expect(input.current().hop).toBe(true);
    expect(input.current().hop).toBe(true);
    navigator.pads = [pad({})];
    expect(input.current().hop).toBe(false);
  });

  it("the left face button is the item button: used on the tick it goes down, held after", () => {
    const navigator = fakeNavigator(pad({ pressed: [2] }));
    const input = gamepadKartInput(navigator);
    expect(input.current()).toMatchObject({ useItem: true, holdItem: true });
    expect(input.current()).toMatchObject({ useItem: false, holdItem: true });
    navigator.pads = [pad({})];
    expect(input.current()).toMatchObject({ useItem: false, holdItem: false });
    navigator.pads = [pad({ pressed: [2] })];
    expect(input.current().useItem).toBe(true);
  });

  it("the top face button looks behind while held and is not part of the kart input", () => {
    const navigator = fakeNavigator(pad({ pressed: [3] }));
    const input = gamepadKartInput(navigator);
    expect(input.lookingBehind()).toBe(true);
    expect(input.current()).toEqual(IDLE_INPUT);
    expect(input.lookingBehind()).toBe(true);
    navigator.pads = [pad({})];
    expect(input.lookingBehind()).toBe(false);
    expect(gamepadKartInput(fakeNavigator()).lookingBehind()).toBe(false);
  });

  it("reads the first connected gamepad", () => {
    const navigator = fakeNavigator(
      null,
      pad({ rightTrigger: 1, connected: false }),
      pad({ leftTrigger: 1 }),
      pad({ rightTrigger: 1 }),
    );
    expect(gamepadKartInput(navigator).current()).toMatchObject({ throttle: 0, brake: 1 });
  });
});
