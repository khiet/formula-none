import { describe, expect, it } from "vitest";
import { keyboardKartInput } from "./keyboard";

/** The slice of a DOM EventTarget the keyboard mapping listens to. */
function fakeKeyboard() {
  type KeyListener = (event: { key: string; preventDefault(): void }) => void;
  const listeners = new Map<string, KeyListener | (() => void)>();
  return {
    addEventListener(type: string, listener: KeyListener | (() => void)) {
      listeners.set(type, listener);
    },
    blur() {
      (listeners.get("blur") as (() => void) | undefined)?.();
    },
    /** Returns whether the key's default browser action was prevented. */
    press(key: string): boolean {
      let prevented = false;
      (listeners.get("keydown") as KeyListener | undefined)?.({
        key,
        preventDefault() {
          prevented = true;
        },
      });
      return prevented;
    },
    release(key: string) {
      (listeners.get("keyup") as KeyListener | undefined)?.({ key, preventDefault() {} });
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

  it("stops arrow keys scrolling the page but leaves other keys alone", () => {
    const keyboard = fakeKeyboard();
    keyboardKartInput(keyboard);
    expect(keyboard.press("ArrowUp")).toBe(true);
    expect(keyboard.press("Tab")).toBe(false);
  });

  it("losing focus releases every held key", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("ArrowUp");
    keyboard.blur();
    expect(input.current()).toEqual({ steer: 0, throttle: 0, brake: 0 });
  });

  it("releasing every key returns to idle", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("ArrowUp");
    keyboard.release("ArrowUp");
    expect(input.current()).toEqual({ steer: 0, throttle: 0, brake: 0 });
  });
});
