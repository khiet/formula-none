// What the race sounds like (ADR 0008): the engine's pitch from the own
// kart's speed and its rocket start's charge, and each SFX from the change
// between two readings of the own kart. Presentation, so it reads what the
// sim reports and never the inputs: a hop sounds when the kart leaves the
// ground, not when the button goes down, and the countdown beeps as the
// count the Instrument panel shows changes, never from a clock of its own.

import type { KartTransform, MiniTurbo, Phase, Standing } from "./bridge";
import type { Item, Start } from "./generated/protocol";
import { type ItemSlot, phaseReadout } from "./instrument-panel";

/** The sounds a race fires, each from the change that calls for it. */
export type SoundCue =
  | "countdownBeep"
  | "go"
  | "hop"
  | "miniTurbo"
  | "itemPickup"
  | "itemFire"
  | "hit"
  | "sputter"
  | "lap";

/**
 * What the own kart sounds like at one moment. The item is the slot's,
 * not the transform's: in a room's race the slot follows the server's item
 * events while the transform repeats the last snapshot's item (ADR 0009).
 */
export interface SoundReading {
  readonly speed: number;
  readonly airborne: boolean;
  /** The tier the drift under way has charged; null off a drift. */
  readonly miniTurbo: MiniTurbo | null;
  readonly item: Item | null;
  readonly spunOut: boolean;
  /** Seconds of boost left while one holds the kart over top speed; null off a boost. */
  readonly boostSeconds: number | null;
  readonly lap: number;
  readonly finished: boolean;
  /** The count the Instrument panel shows: 3, 2, 1, GO, or nothing the rest of the race. */
  readonly count: string;
  /** The rocket start's charge, the tier the hold so far has reached, while the countdown runs; null from Go. */
  readonly charge: Start | null;
  readonly stalled: boolean;
}

/** The own kart's reading from its transform, standing, item slot and the race's phase this frame. */
export function soundReading(
  kart: KartTransform,
  standing: Standing,
  slot: ItemSlot,
  phase: Phase,
): SoundReading {
  return {
    speed: kart.speed,
    airborne: kart.airborne,
    miniTurbo: kart.drift?.miniTurbo ?? null,
    item: slot.item,
    spunOut: kart.spinOutSeconds !== null,
    boostSeconds: kart.boostSeconds,
    lap: standing.lap,
    finished: standing.finishTimeSeconds !== null,
    count: phaseReadout(phase),
    charge: phase.kind === "countdown" ? kart.start : null,
    stalled: kart.stallSeconds !== null,
  };
}

/** Only a drift at one of these tiers has a mini-turbo to release. */
const CHARGED: readonly MiniTurbo[] = ["blue", "orange"];
/** Only a hold at one of these tiers has a start boost to release at Go. */
const BOOSTING: readonly Start[] = ["weakBoost", "fullBoost"];
/** The counts the beep marks; GO has its own tone. */
const BEEPED = ["3", "2", "1"];

/**
 * The cues the change from `before` to `after` calls for, in a fixed
 * order: the beep or the GO tone as the count changes, which the first
 * reading counts for, since the 3 is up from the first frame; the hop as
 * the kart leaves the ground; the mini-turbo as a charged drift ends and a
 * boost starts or lengthens (a drift a spin-out, a respawn or lost speed
 * ends has no release to hear, and a release that cuts a pad's longer
 * boost short is not heard) or as a rocket start's charge becomes its
 * boost at Go; the pickup as the slot fills and the fire as it empties;
 * the hit as a spin-out begins; the sputter as a stall begins; the lap as
 * the count rises or the kart finishes. Nothing else from the first
 * reading.
 */
export function soundCues(before: SoundReading | undefined, after: SoundReading): SoundCue[] {
  const cues: SoundCue[] = [];
  if (after.count !== (before?.count ?? "")) {
    if (BEEPED.includes(after.count)) cues.push("countdownBeep");
    if (after.count === "GO") cues.push("go");
  }
  if (before === undefined) return cues;
  if (!before.airborne && after.airborne) cues.push("hop");
  const charged = before.miniTurbo !== null && CHARGED.includes(before.miniTurbo);
  const boostBegan =
    after.boostSeconds !== null &&
    (before.boostSeconds === null || after.boostSeconds > before.boostSeconds);
  const rocketStarted =
    before.charge !== null && BOOSTING.includes(before.charge) && after.charge === null;
  if (((charged && after.miniTurbo === null) || rocketStarted) && boostBegan) {
    cues.push("miniTurbo");
  }
  if (before.item === null && after.item !== null) cues.push("itemPickup");
  if (before.item !== null && after.item === null) cues.push("itemFire");
  if (!before.spunOut && after.spunOut) cues.push("hit");
  if (!before.stalled && after.stalled) cues.push("sputter");
  if (after.lap > before.lap || (!before.finished && after.finished)) cues.push("lap");
  return cues;
}

/** The engine note at a standstill and at top speed. */
const ENGINE_IDLE_HZ = 70;
const ENGINE_TOP_HZ = 260;
/**
 * Headroom over top speed for the boosts; the note climbs no further. A
 * Presentation ceiling, chosen here rather than read from the sim's boost
 * factors (ADR 0014).
 */
const ENGINE_MAX_SPEED_FRACTION = 1.5;
/**
 * How far up the note a rocket start's charge revs the engine on the grid,
 * as a fraction of top speed, by tier. The stall revs as the full boost
 * does: the hold tipping into a stall gives no warning.
 */
const CHARGE_SPEED_FRACTION: Readonly<Record<Start, number>> = {
  clean: 0,
  weakBoost: 0.4,
  fullBoost: 0.8,
  stall: 0.8,
};

/**
 * The engine note's pitch for `speed`, forward or backing up: idle at a
 * standstill, linear in speed over `topSpeed`, and flat past the boosts'
 * headroom; or the charge's rev while the kart charges its rocket start,
 * whichever is higher.
 */
export function enginePitchHz(
  speed: number,
  topSpeed: number,
  charge: Start | null = null,
): number {
  const revved = charge === null ? 0 : CHARGE_SPEED_FRACTION[charge];
  const fraction = Math.min(
    Math.max(Math.abs(speed) / topSpeed, revved),
    ENGINE_MAX_SPEED_FRACTION,
  );
  return ENGINE_IDLE_HZ + (ENGINE_TOP_HZ - ENGINE_IDLE_HZ) * fraction;
}
