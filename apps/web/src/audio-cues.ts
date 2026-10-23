// What the race sounds like (ADR 0008): the engine's pitch from the own
// kart's speed, and each SFX from the change between two readings of the
// own kart. Presentation, so it reads what the sim reports and never the
// inputs: a hop sounds when the kart leaves the ground, not when the
// button goes down.

import type { KartTransform, MiniTurbo, Standing } from "./bridge";
import type { Item } from "./generated/protocol";
import type { ItemSlot } from "./instrument-panel";

/** The sounds a race fires, each from the change that calls for it. */
export type SoundCue = "hop" | "miniTurbo" | "itemPickup" | "itemFire" | "hit" | "lap";

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
}

/** The own kart's reading from its transform, standing and item slot this frame. */
export function soundReading(
  kart: KartTransform,
  standing: Standing,
  slot: ItemSlot,
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
  };
}

/** Only a drift at one of these tiers has a mini-turbo to release. */
const CHARGED: readonly MiniTurbo[] = ["blue", "orange"];

/**
 * The cues the change from `before` to `after` calls for, in a fixed
 * order: the hop as the kart leaves the ground; the mini-turbo as a charged
 * drift ends and a boost starts or lengthens (a drift a spin-out, a respawn
 * or lost speed ends has no release to hear, and a release that cuts a
 * pad's longer boost short is not heard); the pickup as the slot fills and
 * the fire as it empties; the hit as a spin-out begins; the lap as the
 * count rises or the kart finishes. Nothing from the first reading.
 */
export function soundCues(before: SoundReading | undefined, after: SoundReading): SoundCue[] {
  if (before === undefined) return [];
  const cues: SoundCue[] = [];
  if (!before.airborne && after.airborne) cues.push("hop");
  const charged = before.miniTurbo !== null && CHARGED.includes(before.miniTurbo);
  const boostBegan =
    after.boostSeconds !== null &&
    (before.boostSeconds === null || after.boostSeconds > before.boostSeconds);
  if (charged && after.miniTurbo === null && boostBegan) cues.push("miniTurbo");
  if (before.item === null && after.item !== null) cues.push("itemPickup");
  if (before.item !== null && after.item === null) cues.push("itemFire");
  if (!before.spunOut && after.spunOut) cues.push("hit");
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
 * The engine note's pitch for `speed`, forward or backing up: idle at a
 * standstill, linear in speed over `topSpeed`, and flat past the boosts'
 * headroom.
 */
export function enginePitchHz(speed: number, topSpeed: number): number {
  const fraction = Math.min(Math.abs(speed) / topSpeed, ENGINE_MAX_SPEED_FRACTION);
  return ENGINE_IDLE_HZ + (ENGINE_TOP_HZ - ENGINE_IDLE_HZ) * fraction;
}
