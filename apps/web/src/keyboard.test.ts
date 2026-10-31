import { describe, expect, it } from "vitest";
import { IDLE_INPUT } from "./input";
import { keyboardKartInput } from "./keyboard";

/** The slice of a DOM EventTarget the keyboard mapping listens to. */
function fakeKeyboard() {
  type KeyListener = (event: { key: string; preventDefault(): void }) => void;
  const listeners = new Map<string, KeyListener | (() => void)>();
  return {
    addEventListener(
      type: string,
      listener: KeyListener | (() => void),
      options?: AddEventListenerOptions,
    ) {
      listeners.set(type, listener);
      // As the DOM does: an aborted signal drops the listener.
      options?.signal?.addEventListener("abort", () => listeners.delete(type));
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

describe("keys map to one kart input", () => {
  it("is idle until a key is held", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    expect(input.current()).toEqual(IDLE_INPUT);
  });

  it("up is throttle, down is brake, left and right steer", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);

    keyboard.press("ArrowUp");
    keyboard.press("ArrowRight");
    expect(input.current()).toEqual({ ...IDLE_INPUT, steer: 1, throttle: 1 });

    keyboard.release("ArrowRight");
    keyboard.press("ArrowLeft");
    keyboard.press("ArrowDown");
    expect(input.current()).toEqual({ ...IDLE_INPUT, steer: -1, throttle: 1, brake: 1 });
  });

  it("WASD drive the kart as the arrows do, whichever case the key arrives in", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);

    keyboard.press("w");
    keyboard.press("d");
    expect(input.current()).toEqual({ ...IDLE_INPUT, steer: 1, throttle: 1 });

    // Shift held for the item button turns the letters upper case.
    keyboard.release("D");
    keyboard.press("A");
    keyboard.press("S");
    expect(input.current()).toMatchObject({ steer: -1, throttle: 1, brake: 1 });
    keyboard.release("W");
    expect(input.current().throttle).toBe(0);
  });

  it("Q brakes too, alone or with the throttle held, so the left hand can brake drift", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);

    keyboard.press("q");
    expect(input.current()).toEqual({ ...IDLE_INPUT, brake: 1 });

    keyboard.press("w");
    keyboard.press("d");
    keyboard.press(" ");
    expect(input.current()).toMatchObject({ steer: 1, throttle: 1, brake: 1, hop: true });

    // Shift held for the item button turns the letter upper case.
    keyboard.release("Q");
    expect(input.current().brake).toBe(0);
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

  it("an arrow and its letter held together are one press", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("ArrowUp");
    keyboard.press("w");
    expect(input.current().throttle).toBe(1);
    keyboard.release("w");
    expect(input.current().throttle).toBe(1);
  });

  it("space is the hop button, held while down", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press(" ");
    expect(input.current().hop).toBe(true);
    expect(input.current().hop).toBe(true);
    keyboard.release(" ");
    expect(input.current().hop).toBe(false);
  });

  it("shift is the item button: used on the tick it goes down, held after", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("Shift");
    expect(input.current()).toMatchObject({ useItem: true, holdItem: true });
    expect(input.current()).toMatchObject({ useItem: false, holdItem: true });
    keyboard.release("Shift");
    expect(input.current()).toMatchObject({ useItem: false, holdItem: false });
    keyboard.press("Shift");
    expect(input.current().useItem).toBe(true);
  });

  it("a held key's repeat does not press the item button again", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("Shift");
    input.current();
    keyboard.press("Shift");
    expect(input.current()).toMatchObject({ useItem: false, holdItem: true });
  });

  it("a tap between two ticks still lands on the next one", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press(" ");
    keyboard.release(" ");
    keyboard.press("Shift");
    keyboard.release("Shift");
    expect(input.current()).toMatchObject({ hop: true, useItem: true, holdItem: true });
    expect(input.current()).toMatchObject({ hop: false, useItem: false, holdItem: false });
  });

  it("stops the mapped keys scrolling the page but leaves other keys alone", () => {
    const keyboard = fakeKeyboard();
    keyboardKartInput(keyboard);
    expect(keyboard.press("ArrowUp")).toBe(true);
    expect(keyboard.press(" ")).toBe(true);
    expect(keyboard.press("Tab")).toBe(false);
  });

  it("stops listening once its signal is aborted, so a Scene that goes away takes its keys", () => {
    const keyboard = fakeKeyboard();
    const listeners = new AbortController();
    const input = keyboardKartInput(keyboard, listeners.signal);

    keyboard.press("ArrowUp");
    expect(input.current().throttle).toBe(1);
    listeners.abort();
    keyboard.press("ArrowDown");
    keyboard.release("ArrowUp");
    expect(input.current()).toEqual({ ...IDLE_INPUT, throttle: 1 });
  });

  it("losing focus releases every held key", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("ArrowUp");
    keyboard.press("Shift");
    keyboard.blur();
    expect(input.current()).toEqual(IDLE_INPUT);
  });

  it("E looks behind while held, in either case, and is not part of the kart input", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    expect(input.lookingBehind()).toBe(false);
    expect(keyboard.press("e")).toBe(true);
    expect(input.lookingBehind()).toBe(true);
    expect(input.current()).toEqual(IDLE_INPUT);
    expect(input.lookingBehind()).toBe(true);
    keyboard.release("e");
    expect(input.lookingBehind()).toBe(false);
    // Shift held for the item button turns the letter upper case.
    keyboard.press("E");
    expect(input.lookingBehind()).toBe(true);
    keyboard.release("E");
    expect(input.lookingBehind()).toBe(false);
  });

  it("losing focus stops looking behind", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("e");
    keyboard.blur();
    expect(input.lookingBehind()).toBe(false);
  });

  it("releasing every key returns to idle", () => {
    const keyboard = fakeKeyboard();
    const input = keyboardKartInput(keyboard);
    keyboard.press("ArrowUp");
    input.current();
    keyboard.release("ArrowUp");
    expect(input.current()).toEqual(IDLE_INPUT);
  });
});
