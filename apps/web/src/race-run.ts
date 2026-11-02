import type {
  Coin,
  Hazard,
  ItemBox,
  KartInput,
  KartReadout,
  Phase,
  Projectile,
  Standing,
  Wheel,
} from "./bridge";
import type { Item } from "./generated/protocol";
import type { ReplayRecorder } from "./replay";

/**
 * The item a kart's slot holds ready to fire, as a sim readout or a
 * snapshot has it; null while the slot is empty or still rolling.
 */
export function landedItem(slot: KartReadout["itemSlot"]): Item | null {
  return typeof slot === "object" && "held" in slot ? slot.held : null;
}

/**
 * The item rolling into a kart's slot as a sim readout or a snapshot has
 * it, against a roll of `rollS`; null while the slot is empty or held.
 */
export function rollingOf(slot: KartReadout["itemSlot"], rollS: number): RollingItem | null {
  if (typeof slot !== "object" || !("rolling" in slot)) return null;
  return { item: slot.rolling.item, elapsedS: rollS - slot.rolling.rollLeftS, rollS };
}

/**
 * The item rolling into the local kart's slot (ADR 0070): the item it
 * lands on, how long it has rolled, held at the roll's length while a
 * room's client waits for the snapshot that lands it, and the roll's
 * length, so the reel and its clicks run off the run's clock.
 */
export interface RollingItem {
  readonly item: Item;
  readonly elapsedS: number;
  readonly rollS: number;
}

/**
 * The local kart's item slot as the run reports it: the item the server
 * says is held, the item rolling in, whether the fire is showing,
 * predicted from the press or confirmed by the server's event, whether
 * the landing's flash is showing, and the boxes the kart has taken.
 */
export interface ItemSlot {
  readonly item: Item | null;
  readonly rolling: RollingItem | null;
  readonly firing: boolean;
  readonly landing: boolean;
  /** Every box the kart has taken this race, counted up whether it rolled an item or not. */
  readonly boxesTaken: number;
}

/** The slot of a race with no item taken yet. */
export const EMPTY_SLOT: ItemSlot = {
  item: null,
  rolling: null,
  firing: false,
  landing: false,
  boxesTaken: 0,
};

/**
 * An Air Horn's blast under way: a fire the run reported, numbered in the
 * order the run heard of them, whose it was, where the run had that kart
 * at the fire, and how long ago it went off on the run's clock. The View
 * grows its ring from this and the audio sounds it once; what the blast
 * did to the karts reaches the readout as every spin-out does.
 */
export interface Blast {
  readonly id: number;
  readonly kart: number;
  readonly position: readonly [number, number, number];
  readonly elapsedS: number;
}

/** How long a blast is reported after its fire: the ring's growth and its fade. Presentation, so not the sim's to tune. */
export const BLAST_S = 0.5;

/**
 * A run's record of the horn blasts under way, whichever run sources the
 * fires: each numbered as it is reported, aged a tick at a time on the
 * run's clock and dropped once the blast's time is up.
 */
export interface BlastLedger {
  /** Report `kart`'s fire where it was drawn. */
  fired(kart: number, position: readonly [number, number, number]): void;
  /** Age every blast a tick, dropping those whose time is up. */
  tick(): void;
  /** The blasts under way, oldest first. */
  blasts(): Blast[];
}

export function blastLedger(ticksPerSecond: number): BlastLedger {
  const running: { blast: Blast; ticks: number }[] = [];
  const lifeTicks = Math.round(BLAST_S * ticksPerSecond);
  let reported = 0;
  return {
    fired(kart, position) {
      reported++;
      running.push({ blast: { id: reported, kart, position, elapsedS: 0 }, ticks: 0 });
    },
    tick() {
      for (const entry of running) entry.ticks++;
      while ((running[0]?.ticks ?? 0) >= lifeTicks) running.shift();
    },
    blasts: () =>
      running.map(({ blast, ticks }) => ({ ...blast, elapsedS: ticks / ticksPerSecond })),
  };
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
  /** Whether each coin, in the sidecar's order, is there to pick up. */
  readonly coinsPresent: readonly boolean[];
  /** The hazards on the track where the View draws them. */
  readonly hazards: readonly Hazard[];
  /** The projectiles in flight where the View draws them. */
  readonly projectiles: readonly Projectile[];
  /** The runaway wheel at each of the track's crossings, in the sidecar's order. */
  readonly wheels: readonly Wheel[];
  /** The horn blasts under way, every kart's, oldest first. */
  readonly blasts: readonly Blast[];
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
  /** The track's coins, in the sidecar's order; fixed for the race. */
  coins(): readonly Coin[];
  /** The run as a replay fixture, when the rules ran here to record. */
  readonly replay?: ReplayRecorder;
  /** Let go of whatever the run follows; called when the Scene is disposed. */
  dispose?(): void;
}

/** How long the item slot shows the fire after the item leaves it. Presentation, so not the sim's to tune. */
export const ITEM_FIRE_FLASH_S = 0.5;

/** How long the slot flashes as a rolled item lands. Presentation, so not the sim's to tune. */
export const ITEM_LANDING_FLASH_S = 0.11;

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
