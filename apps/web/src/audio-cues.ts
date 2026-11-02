// What the race sounds like (ADR 0025): each SFX from the change between
// two readings of the own kart, and each loop from the state one reading
// holds. Cues are edges, loops are state. The engine has a module of its
// own, `audio-engine.ts`. Presentation, so it reads what the sim reports
// and never the inputs: a hop sounds when the kart leaves the ground, not
// when the button goes down, and the countdown beeps as the race's count
// changes, never from a clock of its own. The star's music is the one
// sound that hears another kart (ADR 0075): the reading carries the
// nearest star kart's run, and the music follows the own run first.

import type { KartReadout, MiniTurbo, Surface } from "./bridge";
import type { Item, Start } from "./generated/protocol";
import { reelFrame } from "./item-reel";
import { type RaceCount, type RaceReadout, raceCount } from "./race-run";

/** The sounds a race fires, each from the change that calls for it. */
export type SoundCue =
  | "countdownBeep"
  | "go"
  | "hop"
  | "miniTurbo"
  | "itemPickup"
  | "itemRollTick"
  | "itemRolled"
  | "coinPickup"
  | "coinLoss"
  | "itemFire"
  | "hit"
  | "sputter"
  | "lap"
  | "tierChime"
  | "boostPad"
  | "wrongWay"
  | "finish"
  | "starFire"
  | "starEnd";

/** The sounds that run while a state holds: the drift skid and the two run-off surfaces. */
export type SoundLoop = "driftSkid" | "grass" | "sand";

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
  readonly coins: number;
  readonly spunOut: boolean;
  /** Seconds of boost left while one holds the kart over top speed; null off a boost. */
  readonly boostSeconds: number | null;
  readonly lap: number;
  readonly finished: boolean;
  /** The race's count, the one the Instrument panel shows. */
  readonly count: RaceCount;
  /** The rocket start's charge, the tier the hold so far has reached, while the countdown runs; null from Go. The engine revs by it on the grid. */
  readonly charge: Start | null;
  readonly stalled: boolean;
  readonly surface: Surface;
  /** Whether a drift is under way, at any tier; the skid runs on this, the chime on `miniTurbo`. */
  readonly drifting: boolean;
  readonly wrongWay: boolean;
  /** Every box the kart has taken; the pickup sounds as it rises, whatever the box gave. */
  readonly boxesTaken: number;
  /** Whether an item rolls in the slot; the landing sounds as it stops with an item held. */
  readonly rolling: boolean;
  /** The icon the slot's reel has stepped to while an item rolls; null while none does. */
  readonly reelStep: number | null;
  /** Seconds of the own kart's star run left; null off a run. */
  readonly starSeconds: number | null;
  /** The nearest other kart on a star run, by distance; null while none is. */
  readonly nearestStar: NearestStar | null;
}

/** Another kart on a star run, as the own kart hears it. */
export interface NearestStar {
  readonly kart: number;
  /** Seconds of its run left. */
  readonly seconds: number;
  /** Metres from the own kart, along the ground. */
  readonly metres: number;
}

/**
 * The reading of `kart`, the own kart, from its readout and standing in
 * `race`, the item slot and the phase. Throws when `race` has no such kart.
 */
export function soundReading(race: RaceReadout, kart: number): SoundReading {
  const readout = race.karts[kart];
  const standing = race.standings[kart];
  if (readout === undefined || standing === undefined) {
    throw new RangeError(`kart ${kart} is not in the race`);
  }
  const { phase, itemSlot } = race;
  return {
    speed: readout.speed,
    airborne: readout.airborne,
    miniTurbo: readout.drift !== null ? readout.miniTurbo : null,
    item: itemSlot.item,
    coins: readout.coins,
    spunOut: readout.spinOutS !== null,
    boostSeconds: readout.boostS,
    lap: standing.lap,
    finished: standing.finishTimeS !== null,
    count: raceCount(phase),
    charge: phase.kind === "countdown" ? readout.start : null,
    stalled: readout.stallS !== null,
    surface: readout.surface,
    drifting: readout.drift !== null,
    wrongWay: standing.wrongWay,
    boxesTaken: itemSlot.boxesTaken,
    rolling: itemSlot.rolling !== null,
    reelStep:
      itemSlot.rolling === null
        ? null
        : reelFrame(itemSlot.rolling.elapsedS, itemSlot.rolling.rollS).step,
    starSeconds: readout.starS,
    nearestStar: nearestStar(race.karts, kart),
  };
}

/** The nearest kart but `kart` that is on a star run, or null while none is. */
function nearestStar(karts: readonly KartReadout[], kart: number): NearestStar | null {
  const own = karts[kart];
  if (own === undefined) return null;
  let nearest: NearestStar | null = null;
  karts.forEach((other, slot) => {
    if (slot === kart || other.starS === null) return;
    const metres = Math.hypot(
      other.position[0] - own.position[0],
      other.position[2] - own.position[2],
    );
    if (nearest === null || metres < nearest.metres) {
      nearest = { kart: slot, seconds: other.starS, metres };
    }
  });
  return nearest;
}

/** Only a drift at one of these tiers has a mini-turbo to release. */
const CHARGED: readonly MiniTurbo[] = ["orange", "red"];
/** Only a hold at one of these tiers has a start boost to release at Go. */
const BOOSTING: readonly Start[] = ["weakBoost", "fullBoost"];
/** The counts the beep marks, one per start light; Go has its own tone. */
const BEEPED: readonly RaceCount[] = [5, 4, 3, 2, 1];
/** How far up the tiers a drift has charged; off a drift and at the first tier alike, nowhere. */
const TIER_RANK: Readonly<Record<MiniTurbo, number>> = { none: 0, orange: 1, red: 2 };
/**
 * A rise in the run's seconds left of more than this is a second star
 * fired; a smaller one is a reconciliation correction, which restarts
 * nothing.
 */
export const STAR_RESTART_RISE_S = 0.5;

/**
 * The cues the change from `before` to `after` calls for, in a fixed
 * order: the beep or the Go tone as the count changes, which the first
 * reading counts for, since the first light is on from the first frame; the hop as
 * the kart leaves the ground; the mini-turbo as a charged drift ends and a
 * boost starts or lengthens (a drift a spin-out, a respawn or lost speed
 * ends has no release to hear, and a release that cuts a pad's longer
 * boost short is not heard) or as a rocket start's charge becomes its
 * boost at Go; the boost pad as a boost starts or lengthens that neither
 * a release nor an item fired explains, since the transform tells boost
 * seconds and not their source; the tier chime as the drift's tier rises,
 * the same chime to orange and to red, the sparks telling which; the
 * pickup as the kart takes a box, whatever it gave; the click as the
 * slot's reel steps on, once a frame however far it went; the landing as
 * the rolled item lands; the fire as the slot empties, the star's own fire
 * in place of the item fire when the item was a star, and again as a
 * second star puts the run's seconds back up by more than a correction
 * could; the coin pickup as
 * the coins carried rise; the hit as a spin-out begins, then the coin loss
 * as the coins it cost fall; the sputter as a stall begins; the wrong
 * way as its flag raises; the star's end as the run runs out, unless the
 * finish ended it; the finish as the kart finishes, else the lap
 * as the count rises. Nothing else from the first reading.
 */
export function soundCues(before: SoundReading | undefined, after: SoundReading): SoundCue[] {
  const cues: SoundCue[] = [];
  if (after.count !== (before?.count ?? null)) {
    if (BEEPED.includes(after.count)) cues.push("countdownBeep");
    if (after.count === "go") cues.push("go");
  }
  if (before === undefined) return cues;
  if (!before.airborne && after.airborne) cues.push("hop");
  const charged = before.miniTurbo !== null && CHARGED.includes(before.miniTurbo);
  const boostBegan =
    after.boostSeconds !== null &&
    (before.boostSeconds === null || after.boostSeconds > before.boostSeconds);
  const rocketStarted =
    before.charge !== null && BOOSTING.includes(before.charge) && after.charge === null;
  const released = ((charged && after.miniTurbo === null) || rocketStarted) && boostBegan;
  const fired = before.item !== null && after.item === null;
  if (released) cues.push("miniTurbo");
  if (boostBegan && !released && !fired) cues.push("boostPad");
  if (tierRank(after.miniTurbo) > tierRank(before.miniTurbo)) cues.push("tierChime");
  if (after.boxesTaken > before.boxesTaken) cues.push("itemPickup");
  if (before.reelStep !== null && after.reelStep !== null && after.reelStep > before.reelStep) {
    cues.push("itemRollTick");
  }
  if (before.rolling && !after.rolling && after.item !== null) cues.push("itemRolled");
  const starRose =
    after.starSeconds !== null &&
    (before.starSeconds === null || after.starSeconds > before.starSeconds + STAR_RESTART_RISE_S);
  if ((fired && before.item === "star") || starRose) cues.push("starFire");
  else if (fired) cues.push("itemFire");
  if (after.coins > before.coins) cues.push("coinPickup");
  if (!before.spunOut && after.spunOut) cues.push("hit");
  if (after.coins < before.coins) cues.push("coinLoss");
  if (!before.stalled && after.stalled) cues.push("sputter");
  if (!before.wrongWay && after.wrongWay) cues.push("wrongWay");
  const finishedNow = !before.finished && after.finished;
  if (before.starSeconds !== null && after.starSeconds === null && !finishedNow) {
    cues.push("starEnd");
  }
  if (finishedNow) cues.push("finish");
  else if (after.lap > before.lap) cues.push("lap");
  return cues;
}

/** Within this far a sound from another kart plays at full level, falling in a straight line to silent at the far distance. */
export const SOUND_NEAR_M = 10;
export const SOUND_FAR_M = 60;
/** Another kart's sounds come through this lowpass, so they sit behind the own kart's. */
export const SOUND_AWAY_LOWPASS_HZ = 1200;

/**
 * The level, 0 to 1, a sound from a kart `metres` away plays at: full
 * within the near distance, a straight line to silent at the far one. The
 * star's music uses it, and the horn will.
 */
export function distanceLevel(metres: number): number {
  if (metres <= SOUND_NEAR_M) return 1;
  if (metres >= SOUND_FAR_M) return 0;
  return 1 - (metres - SOUND_NEAR_M) / (SOUND_FAR_M - SOUND_NEAR_M);
}

/** The star music to play this frame, and how. */
export interface StarMusic {
  /** Whose run it is: null for the own kart's, else the other kart's grid slot. */
  readonly kart: number | null;
  /** How far into the run's music to join; 0 at a fresh fire. */
  readonly secondsIn: number;
  /** The level before the SFX level: 1 for the own run, by distance for another's. */
  readonly level: number;
  /** The lowpass another kart's run comes through; null for the own. */
  readonly lowpassHz: number | null;
}

/**
 * The star music `reading` calls for, with a run lasting `runSeconds`:
 * the own run while it lasts, else the nearest other star kart's, joined
 * at its point in the run and heard by distance, else none. One run's
 * music at a time: up to seven at once would be noise.
 */
export function starMusic(reading: SoundReading, runSeconds: number): StarMusic | null {
  if (reading.starSeconds !== null) {
    return {
      kart: null,
      secondsIn: Math.max(0, runSeconds - reading.starSeconds),
      level: 1,
      lowpassHz: null,
    };
  }
  const nearest = reading.nearestStar;
  if (nearest === null) return null;
  return {
    kart: nearest.kart,
    secondsIn: Math.max(0, runSeconds - nearest.seconds),
    level: distanceLevel(nearest.metres),
    lowpassHz: SOUND_AWAY_LOWPASS_HZ,
  };
}

function tierRank(tier: MiniTurbo | null): number {
  return tier === null ? 0 : TIER_RANK[tier];
}

/**
 * The loops `reading` calls for: the skid while a drift is under way at
 * any tier (the sim starts a drift on landing, so none runs in the air),
 * and the grass or the sand while the kart is on it. The caller starts
 * and stops loops by diffing this set frame to frame.
 */
export function soundLoops(reading: SoundReading): ReadonlySet<SoundLoop> {
  const loops = new Set<SoundLoop>();
  if (reading.drifting) loops.add("driftSkid");
  if (reading.surface === "grass") loops.add("grass");
  if (reading.surface === "sand") loops.add("sand");
  return loops;
}

/** How a loop plays at one speed: its rate over the file as rendered and its gain before the SFX level. */
export interface LoopPlayback {
  readonly rate: number;
  readonly gain: number;
}

/**
 * How each loop's file was rendered and how it moves with speed. The band's
 * centre in hertz at a speed in metres per second sets the rate, over the
 * centre at the rendered speed, so the skid's band climbs with the kart as
 * the page's did; the swell is the fraction of top speed the file's gain
 * followed, where the fraction is capped at the boosts' headroom, and the
 * gain is the swell over the swell at the rendered speed, since the file
 * already carries that one. These repeat the renders in
 * `assets/audio/build.py`, and the two change together.
 */
interface LoopCurve {
  readonly renderedSpeed: number;
  centreHz(speed: number): number;
  swell(fraction: number): number;
}
const LOOP_CURVES: Readonly<Record<SoundLoop, LoopCurve>> = {
  driftSkid: {
    renderedSpeed: 15,
    centreHz: (speed) => 600 + 20 * speed,
    swell: (fraction) => 0.4 + 0.6 * fraction,
  },
  grass: {
    renderedSpeed: 15,
    centreHz: () => 1200,
    swell: (fraction) => 0.3 + 0.7 * fraction,
  },
  sand: {
    renderedSpeed: 15,
    centreHz: () => 1800,
    swell: (fraction) => 0.3 + 0.7 * fraction,
  },
};
/** A loop's rate never leaves this range, whatever the kart's speed. */
const LOOP_RATE_MIN = 0.5;
const LOOP_RATE_MAX = 2;
/**
 * Headroom over top speed for the boosts; a loop swells no further. A
 * Presentation ceiling, chosen here rather than read from the sim's boost
 * factors (ADR 0014).
 */
const LOOP_MAX_SPEED_FRACTION = 1.5;

/**
 * The rate and gain `loop` plays at for `speed`, forward or backing up,
 * against `topSpeed`; both are 1 at the speed the file was rendered at.
 */
export function loopPlayback(loop: SoundLoop, speed: number, topSpeed: number): LoopPlayback {
  const curve = LOOP_CURVES[loop];
  const rate = curve.centreHz(Math.abs(speed)) / curve.centreHz(curve.renderedSpeed);
  const fraction = (at: number) => Math.min(Math.abs(at) / topSpeed, LOOP_MAX_SPEED_FRACTION);
  return {
    rate: Math.min(Math.max(rate, LOOP_RATE_MIN), LOOP_RATE_MAX),
    gain: curve.swell(fraction(speed)) / curve.swell(fraction(curve.renderedSpeed)),
  };
}
