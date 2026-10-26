import type { KartInputSource } from "./input";

/** The slice of the Gamepad API's Gamepad the mapping reads, so tests can fake it. */
export interface GamepadLike {
  readonly connected: boolean;
  readonly axes: readonly number[];
  readonly buttons: readonly { readonly pressed: boolean; readonly value: number }[];
}

/** The slice of `navigator` the mapping polls. */
export interface GamepadSource {
  getGamepads(): readonly (GamepadLike | null)[];
}

// Indices in the W3C "standard" gamepad layout.
const LEFT_STICK_X = 0;
const BOTTOM_FACE_BUTTON = 0;
const LEFT_FACE_BUTTON = 2;
const TOP_FACE_BUTTON = 3;
const LEFT_TRIGGER = 6;
const RIGHT_TRIGGER = 7;
/** Stick deflection read as centred; a resting stick seldom reports exactly zero. */
const STICK_DEAD_ZONE = 0.15;

/**
 * The first connected gamepad, polled when `current()` is called: right
 * trigger throttle, left trigger brake, left stick steer, bottom face button
 * hop, left face button item, top face button looks behind. Browsers expose a pad only after a button
 * press on it, so an untouched pad reads as absent.
 */
export function gamepadKartInput(source: GamepadSource): KartInputSource {
  let itemWasDown = false;
  const connectedPad = () => source.getGamepads().find((pad) => pad?.connected);
  return {
    current() {
      const pad = connectedPad();
      const button = (index: number) => pad?.buttons[index];
      const itemDown = button(LEFT_FACE_BUTTON)?.pressed ?? false;
      const input = {
        steer: steer(pad?.axes[LEFT_STICK_X] ?? 0),
        throttle: button(RIGHT_TRIGGER)?.value ?? 0,
        brake: button(LEFT_TRIGGER)?.value ?? 0,
        hop: button(BOTTOM_FACE_BUTTON)?.pressed ?? false,
        useItem: itemDown && !itemWasDown,
        holdItem: itemDown,
      };
      itemWasDown = itemDown;
      return input;
    },
    lookingBehind: () => connectedPad()?.buttons[TOP_FACE_BUTTON]?.pressed ?? false,
  };
}

/** Deflection past the dead zone, rescaled so full lock is still 1. */
function steer(stickX: number): number {
  const magnitude = Math.abs(stickX);
  if (magnitude < STICK_DEAD_ZONE) {
    return 0;
  }
  return Math.sign(stickX) * Math.min(1, (magnitude - STICK_DEAD_ZONE) / (1 - STICK_DEAD_ZONE));
}
