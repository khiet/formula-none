// The slot machine reel the own item slot runs while an item rolls in it
// (ADR 0070): a strip of the items scrolled up through the slot, changing
// fast at first and slowing to a stop on the item that lands, with a small
// overshoot. Presentation, so the timing is the View's, read off the time
// the roll has run: the panel scrolls by it and the audio clicks on each
// change, so the two never drift apart.

import type { Item } from "./generated/protocol";

/** Each icon on the strip is the next of these after the one above it, so no two in a row match. */
const REEL_ORDER: readonly Item[] = ["boost", "hazard", "homing"];
/** The gap between changes at the roll's start, and how much it grows by its end. */
const REEL_FIRST_GAP_S = 0.05;
const REEL_GAP_GROWTH_S = 0.23;
/** How long the overshoot past the landing icon and back takes, at the end of the roll. */
const REEL_SETTLE_S = 0.25;
/** How far past the landing icon the overshoot reaches, in icons. */
const REEL_OVERSHOOT = 0.16;

/** Where the reel is: the icon it has stepped to, and how far down the strip it shows, in icons. */
export interface ReelFrame {
  readonly step: number;
  readonly position: number;
}

/** The times each change falls at, from 0, the last at the start of the settle. */
const schedules = new Map<number, readonly number[]>();
function schedule(rollS: number): readonly number[] {
  const known = schedules.get(rollS);
  if (known !== undefined) return known;
  const spanS = Math.max(rollS - REEL_SETTLE_S, 0);
  // The gap grows with the square of the time gone; the whole is scaled so
  // the last change lands on the span's end.
  const times = [0];
  for (let at = 0; at < spanS; ) {
    at += REEL_FIRST_GAP_S + REEL_GAP_GROWTH_S * (at / spanS) ** 2;
    times.push(at);
  }
  const last = times.at(-1) ?? 0;
  const scaled = times.map((time) => (last > 0 ? (time * spanS) / last : 0));
  schedules.set(rollS, scaled);
  return scaled;
}

/** The reel `elapsedS` into a roll of `rollS`; past the roll it rests on the landing icon. */
export function reelFrame(elapsedS: number, rollS: number): ReelFrame {
  const times = schedule(rollS);
  const landing = times.length - 1;
  const settleFrom = times[landing] ?? 0;
  if (elapsedS >= settleFrom) {
    const settled = Math.min((elapsedS - settleFrom) / REEL_SETTLE_S, 1);
    return {
      step: landing,
      position: landing + REEL_OVERSHOOT * Math.sin(Math.PI * settled) * (1 - settled),
    };
  }
  let step = 0;
  while ((times[step + 1] ?? Number.POSITIVE_INFINITY) <= elapsedS) step++;
  const from = times[step] ?? 0;
  const to = times[step + 1] ?? from;
  return { step, position: step + (to > from ? (elapsedS - from) / (to - from) : 0) };
}

/**
 * The strip of icons a roll of `rollS` scrolls through to land on
 * `landed`: each a different item from the one before, the landing icon
 * at the reel's last step and one more past it for the overshoot to show.
 */
export function reelStrip(landed: Item, rollS: number): Item[] {
  const landing = schedule(rollS).length - 1;
  const at = REEL_ORDER.indexOf(landed);
  const count = REEL_ORDER.length;
  return Array.from(
    { length: landing + 2 },
    (_, step) => REEL_ORDER[(((at - landing + step) % count) + count) % count] as Item,
  );
}
