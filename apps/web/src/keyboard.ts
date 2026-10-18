import type { KartInput } from "./bridge";

/** The part of a DOM EventTarget the mapping needs, so tests can fake it. */
export interface KeySource {
  addEventListener(
    type: "keydown" | "keyup",
    listener: (event: { readonly key: string; preventDefault(): void }) => void,
  ): void;
  addEventListener(type: "blur", listener: () => void): void;
}

export interface KartInputSource {
  /** The input for the next tick, read from the keys held right now. */
  current(): KartInput;
}

const STEER_LEFT = "ArrowLeft";
const STEER_RIGHT = "ArrowRight";
const THROTTLE = "ArrowUp";
const BRAKE = "ArrowDown";
const KART_KEYS = new Set([STEER_LEFT, STEER_RIGHT, THROTTLE, BRAKE]);

/** Arrow keys to one kart's input, read from the keys held when `current()` is called. */
export function keyboardKartInput(source: KeySource): KartInputSource {
  const held = new Set<string>();
  source.addEventListener("keydown", (event) => {
    if (KART_KEYS.has(event.key)) {
      // Arrow keys scroll the page otherwise.
      event.preventDefault();
      held.add(event.key);
    }
  });
  source.addEventListener("keyup", (event) => {
    held.delete(event.key);
  });
  // Keyup never arrives for a key released in another window.
  source.addEventListener("blur", () => {
    held.clear();
  });
  const axis = (key: string): number => (held.has(key) ? 1 : 0);
  return {
    current: () => ({
      steer: axis(STEER_RIGHT) - axis(STEER_LEFT),
      throttle: axis(THROTTLE),
      brake: axis(BRAKE),
    }),
  };
}
