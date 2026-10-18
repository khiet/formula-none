import { describe, expect, it } from "vitest";
import { keyboardKartInput } from "./keyboard";

/** The slice of a DOM EventTarget the keyboard mapping listens to. */
function fakeKeyboard() {
  const listeners = new Map<string, (event: { key: string; preventDefault(): void }) => void>();
  return {
    addEventListener(
      type: string,
      listener: (event: { key: string; preventDefault(): void }) => void,
    ) {
      listeners.set(type, listener);
    },
    press(key: string) {
      listeners.get("keydown")?.({ key, preventDefault() {} });
    },
    release(key: string) {
      listeners.get("keyup")?.({ key, preventDefault() {} });
    },
  };
}

describe("arrow keys map to one kart input", () => {
  it("is idle until a key is held", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    expect(input.current()).toEqual({ steer: 0, throttle: 0, brake: 0 });
  });

  it("up is throttle, down is brake, left and right steer", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);

    keyboard.press("ArrowUp");
    keyboard.press("ArrowRight");
    expect(input.current()).toEqual({ steer: 1, throttle: 1, brake: 0 });

    keyboard.release("ArrowRight");
    keyboard.press("ArrowLeft");
    keyboard.press("ArrowDown");
    expect(input.current()).toEqual({ steer: -1, throttle: 1, brake: 1 });
  });

  it("opposite steer keys held together cancel out", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("ArrowLeft");
    keyboard.press("ArrowRight");
    expect(input.current().steer).toBe(0);
    keyboard.release("ArrowLeft");
    expect(input.current().steer).toBe(1);
  });

  it("releasing every key returns to idle", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("ArrowUp");
    keyboard.release("ArrowUp");
    expect(input.current()).toEqual({ steer: 0, throttle: 0, brake: 0 });
  });
});
