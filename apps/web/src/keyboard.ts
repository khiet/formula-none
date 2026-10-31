import type { KartInputSource } from "./input";

/** The part of a DOM EventTarget the mapping needs, so tests can fake it. */
export interface KeySource {
  addEventListener(
    type: "keydown" | "keyup",
    listener: (event: { readonly key: string; preventDefault(): void }) => void,
    options?: AddEventListenerOptions,
  ): void;
  addEventListener(type: "blur", listener: () => void, options?: AddEventListenerOptions): void;
}

type Action = "left" | "right" | "throttle" | "brake" | "hop" | "item" | "lookBehind";

/**
 * Arrows or WASD drive, space hops, shift is the item button, E looks
 * behind. Q brakes too, so the throttle can stay held through a brake drift:
 * W and S share a finger. Letters are lower case here.
 */
const KEY_ACTIONS: ReadonlyMap<string, Action> = new Map([
  ["ArrowLeft", "left"],
  ["a", "left"],
  ["ArrowRight", "right"],
  ["d", "right"],
  ["ArrowUp", "throttle"],
  ["w", "throttle"],
  ["ArrowDown", "brake"],
  ["s", "brake"],
  ["q", "brake"],
  [" ", "hop"],
  ["Shift", "item"],
  ["e", "lookBehind"],
]);

/** Shift turns a letter upper case; the key is the same. */
function normalise(key: string): string {
  return key.length === 1 ? key.toLowerCase() : key;
}

/**
 * The keys held when `current()` is called, as one kart's input. A key that
 * went down and up again between two reads counts as held for the next one,
 * so a tap shorter than a tick still lands. Looking behind follows only the
 * keys held, since the camera reads it every frame. Aborting `signal` stops
 * listening, so a Scene that goes away takes its keys with it.
 */
export function keyboardKartInput(source: KeySource, signal?: AbortSignal): KartInputSource {
  const held = new Set<string>();
  const tapped = new Set<string>();
  let itemPressed = false;
  const options: AddEventListenerOptions = signal === undefined ? {} : { signal };
  source.addEventListener(
    "keydown",
    (event) => {
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
    },
    options,
  );
  source.addEventListener(
    "keyup",
    (event) => {
      held.delete(normalise(event.key));
    },
    options,
  );
  // Keyup never arrives for a key released in another window.
  source.addEventListener(
    "blur",
    () => {
      held.clear();
      tapped.clear();
      itemPressed = false;
    },
    options,
  );

  /** Whether a key for `action` is held, or with `withTaps` was tapped since the last read. */
  const down = (action: Action, withTaps = true): boolean => {
    for (const [key, mapped] of KEY_ACTIONS) {
      if (mapped === action && (held.has(key) || (withTaps && tapped.has(key)))) {
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
    lookingBehind: () => down("lookBehind", false),
  };
}
