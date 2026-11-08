import type { KartReadout } from "./bridge";
import type { KartReadout as SimKartReadout } from "./generated/sim";

export type { SimKartReadout };

/**
 * A kart for a test to vary, built on the bridge's one readout type: at
 * rest at the origin on the asphalt, facing -Z, first on lap 1 and out of
 * every mechanic. A field the sim adds to the readout or its readings gets
 * its default here and nowhere else.
 */
export const RESTING_KART: KartReadout = Object.freeze({
  position: Object.freeze([0, 0, 0]) as readonly [number, number, number],
  rotation: Object.freeze([0, 0, 0, 1]) as readonly [number, number, number, number],
  surface: "asphalt",
  speed: 0,
  velocity: [0, 0, 0] as [number, number, number],
  bump: null,
  drift: null,
  miniTurbo: "none",
  itemSlot: "empty",
  coins: 0,
  tyreLife: 100,
  spinOutS: null,
  airborne: false,
  trick: false,
  inPitLane: false,
  stop: null,
  stops: 0,
  boost: null,
  boostS: null,
  boostTier: "none",
  starS: null,
  slipstream: null,
  start: "clean",
  stallS: null,
  racePosition: 1,
  lap: 1,
  checkpoint: 0,
  splineProgress: 0,
  wrongWay: false,
});

/** The resting kart with `overrides`. */
export function kartReadout(overrides: Partial<KartReadout> = {}): KartReadout {
  return Object.freeze({ ...RESTING_KART, ...overrides });
}

/**
 * The resting kart as a snapshot carries it, with `overrides`: the sim's
 * readout without the readings the bridge works out beside it.
 */
export function simKartReadout(overrides: Partial<SimKartReadout> = {}): SimKartReadout {
  const { surface, miniTurbo, boostS, boostTier, position, rotation, ...readout } = RESTING_KART;
  return { ...readout, position: [...position], rotation: [...rotation], ...overrides };
}

/** The rotation that turns a kart `yaw` radians about +Y from its -Z facing. */
export function yawed(yaw: number): readonly [number, number, number, number] {
  return Object.freeze([0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)]) as readonly [
    number,
    number,
    number,
    number,
  ];
}
