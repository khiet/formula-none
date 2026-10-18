import type { KartInputSource } from "./input";

/** The part of a DOM EventTarget the mapping needs, so tests can fake it. */
export interface KeySource {
  addEventListener(
    type: "keydown" | "keyup",
    listener: (event: { readonly key: string; preventDefault(): void }) => void,
  ): void;
  addEventListener(type: "blur", listener: () => void): void;
}

type Action = "left" | "right" | "throttle" | "brake" | "hop" | "item";

/** Arrows or WASD drive, space hops, shift is the item button. Letters are lower case here. */
const KEY_ACTIONS: ReadonlyMap<string, Action> = new Map([
  ["ArrowLeft", "left"],
  ["a", "left"],
  ["ArrowRight", "right"],
  ["d", "right"],
  ["ArrowUp", "throttle"],
  ["w", "throttle"],
  ["ArrowDown", "brake"],
  ["s", "brake"],
  [" ", "hop"],
  ["Shift", "item"],
]);

/** Shift turns a letter upper case; the key is the same. */
function normalise(key: string): string {
  return key.length === 1 ? key.toLowerCase() : key;
}

/**
 * The keys held when `current()` is called, as one kart's input. A key that
 * went down and up again between two reads counts as held for the next one,
 * so a tap shorter than a tick still lands.
 */
export function keyboardKartInput(source: KeySource): KartInputSource {
  const held = new Set<string>();
  const tapped = new Set<string>();
  let itemPressed = false;
  source.addEventListener("keydown", (event) => {
    const key = normalise(event.key);
    if (!KEY_ACTIONS.has(key)) {
      return;
    }
    // Arrows and space scroll the page otherwise.
    event.preventDefault();
    // A held key repeats keydown; only the first one is a press.
    if (!held.has(key)) {
      itemPressed ||= KEY_ACTIONS.get(key) === "item";
      tapped.add(key);
    }
    held.add(key);
  });
  source.addEventListener("keyup", (event) => {
    held.delete(normalise(event.key));
  });
  // Keyup never arrives for a key released in another window.
  source.addEventListener("blur", () => {
    held.clear();
    tapped.clear();
    itemPressed = false;
  });

  const down = (action: Action): boolean => {
    for (const [key, mapped] of KEY_ACTIONS) {
      if (mapped === action && (held.has(key) || tapped.has(key))) {
        return true;
      }
    }
    return false;
  };
  return {
    current() {
      const input = {
        steer: Number(down("right")) - Number(down("left")),
        throttle: Number(down("throttle")),
        brake: Number(down("brake")),
        hop: down("hop"),
        useItem: itemPressed,
        holdItem: down("item"),
      };
      tapped.clear();
      itemPressed = false;
      return input;
    },
  };
}
