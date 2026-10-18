import type { KartInput } from "./bridge";

/** The part of a DOM EventTarget the mapping needs, so tests can fake it. */
export interface KeySource {
  addEventListener(
    type: "keydown" | "keyup",
    listener: (event: { readonly key: string; preventDefault(): void }) => void,
  ): void;
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

/** Arrow keys to one kart's input. Held keys are read, never events, so a tick never misses a key. */
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
  const axis = (key: string): number => (held.has(key) ? 1 : 0);
  return {
    current: () => ({
      steer: axis(STEER_RIGHT) - axis(STEER_LEFT),
      throttle: axis(THROTTLE),
      brake: axis(BRAKE),
    }),
  };
}
