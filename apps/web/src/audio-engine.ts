// The engine's gears, limiter and load (ADR 0025), derived from the own
// kart's speed alone: speed splits into gear bands that widen geometrically,
// so the first shifts come fast and the top gear holds; past top speed a
// boost revs sixth into a limiter; load is inferred from the change in
// speed between readings, so pulling away opens the filter and lifting
// closes it under a crackle; on the grid the revs follow the rocket start's
// charge by tier. Presentation only: it reads what the sim reports and
// drives no audio node. `audio.ts` voices what this reports. The values
// were tuned by ear on the iteration page under `docs/mockups`, which the
// ADR names as the record; retune there first.

import type { Start } from "./generated/protocol";

/** The rig as the page carries it. `limiter` caps the revs; the chop runs while they sit over 1, or while a full grid rev forces it. */
export const ENGINE = {
  gears: 6,
  /** Each gear's speed band over the one below it. */
  bandRatio: 1.4,
  /** The revs an upshift lands on; a band runs from here to 1. */
  dropRatio: 0.45,
  /** The note is cut this long on an upshift. */
  upshiftCutSeconds: 0.03,
  /** A downshift blips the note up by this fraction of its pitch before it settles. */
  downshiftBlip: 0.1,
  /** A downshift waits for speed to fall this fraction of the band's width under its floor, so a steady speed never chatters. */
  hysteresis: 0.05,
  limiter: 1.08,
  limiterHz: 28,
  idleHz: 110,
  topHz: 720,
  /** Between the two sawtooths, in cents. */
  detuneCents: 12,
  /** The square sub's gain under the two sawtooths at 0.5 each. */
  sub: 0.35,
  /** The intake noise's gain at full revs. */
  intake: 0.15,
  filterClosedHz: 400,
  filterOpenHz: 6000,
  /** The level the engine falls to on a full overrun. */
  overrunLevel: 0.6,
  /** The crackle level; a burst's gain is a share of it that grows with the overrun. */
  crackle: 0.5,
} as const;

/**
 * The revs a rocket start's charge holds on the grid, by tier. The full
 * boost and the stall hold the top note with the limiter's chop forced on,
 * as the page does: the hold tipping into a stall gives no warning.
 */
export const GRID_REVS: Readonly<Record<Start, number>> = {
  clean: 0,
  weakBoost: 0.5,
  fullBoost: 1,
  stall: 1,
};

/** Under this speed the engine idles, and the charge may rev it. */
const IDLE_SPEED = 0.5;
/**
 * How long the sim takes to reach top speed at full throttle, so the speed
 * delta reads as a fraction of full load. A Presentation reference, chosen
 * here rather than read from the sim's tuning (ADR 0014); the bridge does
 * not carry the acceleration.
 */
export const FULL_THROTTLE_SECONDS_TO_TOP = 2.5;
/** The load settles toward each reading's at this rate per second. */
const LOAD_RATE = 12;
/** Past this overrun the crackle fires, and never under this speed. */
const CRACKLE_OVERRUN = 0.15;
const CRACKLE_SPEED = 2;
/** How far the chop takes the level down while the limiter holds. */
const LIMITER_DEPTH = 0.5;
/** The filter opens this share of the way from the revs alone; the throttle opens the rest. */
const FILTER_REVS_SHARE = 0.45;
/** A full overrun closes the filter by this much. */
const OVERRUN_FILTER_CLOSE = 0.5;
/** The intake breathes from this share of its gain at idle to all of it at full revs. */
const INTAKE_IDLE_SHARE = 0.3;
/** The intake's band sits this many times the note. */
const INTAKE_BAND_OVER_NOTE = 4;
/** A burst's gain is this share of the crackle level on a full overrun. */
const CRACKLE_BURST_SHARE = 0.6;

/** The own kart at one moment, as the engine hears it. */
export interface EngineSample {
  readonly speed: number;
  /** The rocket start's charge while the countdown runs; null from Go. */
  readonly charge: Start | null;
  /** The audio clock, in seconds; the load reads the speed delta over it. */
  readonly time: number;
}

export interface EngineState {
  /** 1 to `ENGINE.gears`. */
  readonly gear: number;
  /** 0 at idle, 1 at the band's top, up to `ENGINE.limiter` past top speed. */
  readonly revs: number;
  /** -1 on a full overrun, 1 at full throttle, 0 held steady. */
  readonly load: number;
  readonly atLimiter: boolean;
  /** The shift this reading made, if any; the voice cuts on "up" and blips on "down". */
  readonly shift: "up" | "down" | null;
  readonly speed: number;
  readonly time: number;
}

/**
 * The speeds the gears change at, from 0 to `topSpeed`: edge k is
 * `top * (r^k - 1) / (r^N - 1)` for the band ratio r and the gear count N,
 * so each band is r times the one below it.
 */
export function engineBandEdges(topSpeed: number): number[] {
  const { gears, bandRatio } = ENGINE;
  const total = bandRatio ** gears - 1;
  const edges = Array.from(
    { length: gears + 1 },
    (_, k) => (topSpeed * (bandRatio ** k - 1)) / total,
  );
  edges[gears] = topSpeed;
  return edges;
}

/**
 * The engine's state after `sample`, from `before` or from nothing: the
 * gear it holds with hysteresis, the revs in that gear, the load from the
 * speed delta and whether the limiter holds. The first reading picks the
 * gear from speed alone and makes no shift.
 */
export function engineStep(
  before: EngineState | undefined,
  sample: EngineSample,
  topSpeed: number,
): EngineState {
  const edges = engineBandEdges(topSpeed);
  const speed = Math.abs(sample.speed);
  const gear = gearFor(speed, before?.gear ?? 1, edges);
  const floor = edges[gear - 1] ?? 0;
  const ceiling = edges[gear] ?? topSpeed;
  const within = (speed - floor) / (ceiling - floor);
  let revs = ENGINE.dropRatio + (1 - ENGINE.dropRatio) * Math.max(0, within);
  revs = gear === ENGINE.gears ? Math.min(ENGINE.limiter, revs) : Math.min(1, revs);
  if (speed < IDLE_SPEED && gear === 1) revs = 0;
  const gridRevs = speed < IDLE_SPEED && sample.charge !== null ? GRID_REVS[sample.charge] : 0;
  revs = Math.max(revs, gridRevs);
  return {
    gear,
    revs,
    load: loadAfter(before, sample.time, speed, topSpeed),
    atLimiter: revs > 1 + 1e-3 || gridRevs >= 1,
    shift: before === undefined || gear === before.gear ? null : gear > before.gear ? "up" : "down",
    speed,
    time: sample.time,
  };
}

/** The gear `speed` calls for from the one `held`: up past a band's top, down only past the hysteresis margin under its floor. */
function gearFor(speed: number, held: number, edges: readonly number[]): number {
  let gear = Math.min(Math.max(held, 1), ENGINE.gears);
  while (gear < ENGINE.gears && speed > (edges[gear] ?? Number.POSITIVE_INFINITY)) gear++;
  while (gear > 1) {
    const floor = edges[gear - 1] ?? 0;
    const margin = ENGINE.hysteresis * ((edges[gear] ?? floor) - floor);
    if (speed >= floor - margin) break;
    gear--;
  }
  return gear;
}

/** The load settled toward this reading's speed delta over the time since the last; kept as it was with no time passed. */
function loadAfter(
  before: EngineState | undefined,
  time: number,
  speed: number,
  topSpeed: number,
): number {
  if (before === undefined) return 0;
  const dt = time - before.time;
  if (dt <= 0) return before.load;
  const fullThrottle = topSpeed / FULL_THROTTLE_SECONDS_TO_TOP;
  const raw = Math.min(1, Math.max(-1, (speed - before.speed) / dt / fullThrottle));
  return before.load + (raw - before.load) * Math.min(1, dt * LOAD_RATE);
}

/** What the graph should sound like for one state: the targets each node glides to. */
export interface EngineVoice {
  /** The two sawtooths' note. */
  readonly noteHz: number;
  /** The square sub, an octave under. */
  readonly subHz: number;
  /** The intake noise's band centre, riding over the note. */
  readonly intakeHz: number;
  /** The intake noise's gain. */
  readonly intake: number;
  /** The lowpass cutoff: closed at idle, closing on the overrun, open at full throttle and revs. */
  readonly filterHz: number;
  /** The gain after the filter, before the cut; down on the overrun. */
  readonly level: number;
  /** How deep the limiter's chop goes; 0 off the limiter. */
  readonly limiterDepth: number;
  /** The gain of a crackle burst due now; 0 for none. */
  readonly crackle: number;
  /** How deep the lift is, 0 on throttle to 1 on a full overrun; the bursts come closer the deeper it is. */
  readonly overrun: number;
}

export function engineVoice(state: EngineState): EngineVoice {
  const noteHz = ENGINE.idleHz + (ENGINE.topHz - ENGINE.idleHz) * state.revs;
  const revs = Math.min(1, state.revs);
  const onThrottle = Math.max(0, state.load);
  const overrun = Math.max(0, -state.load);
  const open =
    revs *
    (FILTER_REVS_SHARE + (1 - FILTER_REVS_SHARE) * onThrottle) *
    (1 - OVERRUN_FILTER_CLOSE * overrun);
  const crackles = overrun > CRACKLE_OVERRUN && state.speed > CRACKLE_SPEED;
  return {
    noteHz,
    subHz: noteHz / 2,
    intakeHz: noteHz * INTAKE_BAND_OVER_NOTE,
    intake: ENGINE.intake * (INTAKE_IDLE_SHARE + (1 - INTAKE_IDLE_SHARE) * revs),
    filterHz: ENGINE.filterClosedHz + (ENGINE.filterOpenHz - ENGINE.filterClosedHz) * open,
    level: 1 - overrun * (1 - ENGINE.overrunLevel),
    limiterDepth: state.atLimiter ? LIMITER_DEPTH : 0,
    crackle: crackles ? ENGINE.crackle * CRACKLE_BURST_SHARE * overrun : 0,
    overrun,
  };
}
