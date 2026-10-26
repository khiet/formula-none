import type {
  Hazard,
  ItemBox,
  KartInput,
  KartReadout,
  Phase,
  Projectile,
  Standing,
} from "./bridge";
import type { Item } from "./generated/protocol";
import type { ReplayRecorder } from "./replay";

/**
 * The local kart's item slot as the run reports it: the item the server
 * says is held, and whether the fire is showing, predicted from the press
 * or confirmed by the server's event.
 */
export interface ItemSlot {
  readonly item: Item | null;
  readonly firing: boolean;
}

/**
 * What the run says of the race as of the last tick, read once a frame and
 * handed whole to the View, the Instrument panel and the audio.
 */
export interface RaceReadout {
  /** One readout per kart in grid order. */
  readonly karts: readonly KartReadout[];
  /** One standing per kart in grid order. */
  readonly standings: readonly Standing[];
  readonly phase: Phase;
  /** The local kart's item slot. */
  readonly itemSlot: ItemSlot;
  /** Whether each item box, in the sidecar's order, is there to pick up. */
  readonly itemBoxesPresent: readonly boolean[];
  /** The hazards on the track where the View draws them. */
  readonly hazards: readonly Hazard[];
  /** The projectiles in flight where the View draws them. */
  readonly projectiles: readonly Projectile[];
}

/**
 * A race as the Race Scene runs and reads it, wherever the rules run: the
 * sim in this browser for a solo race, or the server's snapshots for a
 * room's (ADR 0009). The Scene ticks it with the local player's input and
 * draws what it reports.
 */
export interface RaceRun {
  readonly humanCount: number;
  readonly localKart: number;
  /** One tick of `dt` seconds with the local player's input: stepped here, or sent to the server. */
  tick(input: KartInput, dt: number): void;
  /** The race as of the last tick. */
  readout(): RaceReadout;
  /** The track's item boxes, in the sidecar's order; fixed for the race. */
  itemBoxes(): readonly ItemBox[];
  /** The run as a replay fixture, when the rules ran here to record. */
  readonly replay?: ReplayRecorder;
  /** Let go of whatever the run follows; called when the Scene is disposed. */
  dispose?(): void;
}

/** How long the item slot shows the fire after the item leaves it. Presentation, so not the sim's to tune. */
export const ITEM_FIRE_FLASH_S = 0.5;

/** How long the count shows Go once the race is on. Presentation, so not the sim's to tune. */
const GO_S = 1;

/** The count over the race: whole seconds left to Go, Go, or nothing. */
export type RaceCount = number | "go" | null;

/**
 * The count at `phase`: the whole seconds left to Go while the countdown
 * runs, Go for the first second of the race, and nothing the rest of the
 * time. The Instrument panel prints it and the audio beeps on it, so the
 * two never disagree.
 */
export function raceCount(phase: Phase): RaceCount {
  switch (phase.kind) {
    case "countdown":
      return Math.ceil(phase.remainingSeconds);
    case "racing":
      return phase.elapsedSeconds < GO_S ? "go" : null;
    case "outro":
    case "finished":
      return null;
  }
}
