import type { KartInput } from "./bridge";

export interface KartInputSource {
  /** The input for the next tick, read from the device as it is right now. */
  current(): KartInput;
}

export const IDLE_INPUT: KartInput = {
  steer: 0,
  throttle: 0,
  brake: 0,
  hop: false,
  useItem: false,
  holdItem: false,
};

/**
 * One input from several devices, so a player picks up the gamepad mid-drive
 * or drops it for the keyboard without choosing either: each axis follows
 * the device pushing it hardest and a button is pressed on any device.
 */
export function combinedKartInput(...sources: readonly KartInputSource[]): KartInputSource {
  return {
    current: () => sources.map((source) => source.current()).reduce(combine, IDLE_INPUT),
  };
}

function combine(a: KartInput, b: KartInput): KartInput {
  return {
    steer: Math.abs(b.steer) > Math.abs(a.steer) ? b.steer : a.steer,
    throttle: Math.max(a.throttle, b.throttle),
    brake: Math.max(a.brake, b.brake),
    hop: a.hop || b.hop,
    useItem: a.useItem || b.useItem,
    holdItem: a.holdItem || b.holdItem,
  };
}
