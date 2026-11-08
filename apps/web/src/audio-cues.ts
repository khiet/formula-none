// What the race sounds like (ADR 0025): each SFX from the change between
// two readings of the own kart, and each loop from the state one reading
// holds. Cues are edges, loops are state. The engine has a module of its
// own, `audio-engine.ts`. Presentation, so it reads what the sim reports
// and never the inputs: a hop sounds when the kart leaves the ground, not
// when the button goes down, and the countdown beeps as the race's count
// changes, never from a clock of its own. Two sounds hear other karts:
// the star's music (ADR 0075), from the nearest star kart's run the
// reading carries, and the Air Horn's blast (ADR 0076), from every other
// kart's blast the reading carries, each heard once by distance. The
// drone warning's beep is the one sound on a clock: it repeats while a
// drone chasing the own kart is within the warning's reach, faster and
// higher as it closes, from the distance the reading carries. The
// slipstream's whoosh (ADR 0077) is the own kart's tow taking, an edge
// like the rest. So is each beat of the own kart's stop (ADR 0082): the
// crew's sounds fire as the stop clock crosses the beats the crew's poses
// read, so none plays for a movement the player cannot see.

import type { KartReadout, MiniTurbo, Stop, StopTier, Surface, Tuning } from "./bridge";
import { stopClock } from "./crew-stance";
import { DRONE_WARNING_M, droneWarned, incomingDroneMetres } from "./drone-warning";
import type { Item, Start } from "./generated/sim";
import { reelFrame } from "./item-reel";
import { groundDistance } from "./kart-frame";
import { OPEN_SKY, type PresentationBlend } from "./presentation-zones";
import { type RaceCount, type RaceReadout, raceCount } from "./race-run";
import { actOf, beatsOf, REDO_STEPS_S, RELEASE } from "./stop-beats";

/** The sounds a race fires, each from the change that calls for it. */
export type SoundCue =
  | "countdownBeep"
  | "go"
  | "hop"
  | "trick"
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
  | "starEnd"
  | "airHorn"
  | "droneBeep"
  | "slipstream"
  | "flypast"
  | "boxChime"
  | "boots"
  | "jackLift"
  | "wheelGun"
  | "wheelOff"
  | "wheelOn"
  | "wheelGunOn"
  | "lollipopSpin"
  | "jackDrop"
  | "releaseChime"
  | "gunStall"
  | "jackFail"
  | "wheelGunAct"
  | "wheelGunRedo";

/** The loops that run while a state of the kart holds: the drift skid and the two run-off surfaces. */
export type SpeedLoop = "driftSkid" | "grass" | "sand";
/** The loops a presentation zone brings up (ADR 0098): the harbour's water and a stand's crowd. */
export type AmbienceLoop = "harbour" | "crowd";
/** The sounds that run while a state holds. */
export type SoundLoop = SpeedLoop | AmbienceLoop;

/**
 * What the own kart sounds like at one moment. The item is the slot's,
 * not the transform's: in a room's race the slot follows the server's item
 * events while the transform repeats the last snapshot's item (ADR 0009).
 */
export interface SoundReading {
  readonly speed: number;
  readonly airborne: boolean;
  /** Whether a trick waits for the landing; it sounds as the press earns it, the landing's boost as a pad's. */
  readonly trick: boolean;
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
  /** Whether a slipstream tows the own kart; the whoosh sounds as it takes. Not a boost: the boost pad's inference never reads it. */
  readonly towed: boolean;
  /** Whether the BOX call is showing; the chime sounds as it comes up. */
  readonly boxCall: boolean;
  /** The own kart's stop in its pit box (ADR 0082); null while it has none. */
  readonly stop: HeardStop | null;
  /** The nearest other kart on a star run, by distance; null while none is. */
  readonly nearestStar: NearestStar | null;
  /** The other karts' horn blasts under way, by distance; never the own kart's, whose fire the slot sounds. */
  readonly blasts: readonly HeardBlast[];
  /** Metres to the nearest homing projectile chasing the own kart; null while none is. */
  readonly incomingDroneMetres: number | null;
  /** The presentation zone's blend where the own kart is (ADR 0098): the engine's echo and the ambience's levels. */
  readonly presentation: PresentationBlend;
  /** The wall clock of the display jets' latest cue (ADR 0099); null before the first. */
  readonly flypastCuedMs: number | null;
  /** Whether the own kart is on its last lap (ADR 0100): the race music lifts. */
  readonly finalLap: boolean;
}

/** The own kart's stop, as its crew's sounds read it. */
export interface HeardStop {
  readonly tier: StopTier;
  readonly stationaryS: number;
  /** The stop clock: negative through the guide onto the mark, the seconds on it after. */
  readonly clock: number;
  /** A clean stop's longest, where the tier's act starts. */
  readonly actFromS: number;
}

/** Another kart's horn blast, as the own kart hears it. */
export interface HeardBlast {
  /** The run's number for the blast, so each is heard once however long it is reported. */
  readonly id: number;
  readonly kart: number;
  /** Metres from the own kart, along the ground. */
  readonly metres: number;
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
 * `race`, the item slot and the phase, with `tuning.stopCleanS` a clean
 * stop's longest, `boxCall` whether the BOX call shows, `presentation`
 * the zone's blend where the kart is and `finalLap` whether the kart is on
 * its last lap. Throws when `race` has no such kart.
 */
export function soundReading(
  race: RaceReadout,
  kart: number,
  tuning: Pick<Tuning, "stopCleanS">,
  boxCall = false,
  presentation: PresentationBlend = OPEN_SKY,
  flypastCuedMs: number | null = null,
  finalLap = false,
): SoundReading {
  const readout = race.karts[kart];
  const standing = race.standings[kart];
  if (readout === undefined || standing === undefined) {
    throw new RangeError(`kart ${kart} is not in the race`);
  }
  const { phase, itemSlot } = race;
  return {
    speed: readout.speed,
    airborne: readout.airborne,
    trick: readout.trick,
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
    towed: readout.slipstream !== null && "tow" in readout.slipstream,
    boxCall,
    stop: heardStop(readout.stop, tuning.stopCleanS),
    nearestStar: nearestStar(race.karts, kart),
    blasts: race.blasts
      .filter((blast) => blast.kart !== kart)
      .map((blast) => ({
        id: blast.id,
        kart: blast.kart,
        metres: groundDistance({ position: blast.position }, readout),
      })),
    incomingDroneMetres: incomingDroneMetres(race.projectiles, kart, readout),
    presentation,
    flypastCuedMs,
    finalLap,
  };
}

function heardStop(stop: Stop | null, actFromS: number): HeardStop | null {
  if (stop === null) return null;
  return { tier: stop.tier, stationaryS: stop.stationaryS, clock: stopClock(stop), actFromS };
}

/** The nearest kart but `kart` that is on a star run, or null while none is. */
function nearestStar(karts: readonly KartReadout[], kart: number): NearestStar | null {
  const own = karts[kart];
  if (own === undefined) return null;
  let nearest: NearestStar | null = null;
  karts.forEach((other, slot) => {
    if (slot === kart || other.starS === null) return;
    const metres = groundDistance(other, own);
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
 * the rolled item lands; the fire as the slot empties, the horn's own
 * blast for a horn, except for a star,
 * whose own fire sounds instead as its run begins or as a second star puts
 * the run's seconds back up by more than a correction could; the coin
 * pickup as
 * the coins carried rise; the hit as a spin-out begins, then the coin loss
 * as the coins it cost fall; the sputter as a stall begins; the wrong
 * way as its flag raises; the star's end as the run runs out, unless the
 * finish ended it; the slipstream as a tow takes, and nothing as it holds,
 * is re-armed or runs out (ADR 0077); the jets' swell as each of their
 * cues comes (ADR 0099); the BOX chime as its call comes up
 * (ADR 0080); the crew's boots as the own kart is caught for its stop,
 * each of its beats as the stop clock crosses it (`stopCues`), and the
 * jack's drop and the radio's go as it is let go; the finish as the kart finishes,
 * else the lap as the count rises. Nothing else from the first reading.
 */
export function soundCues(before: SoundReading | undefined, after: SoundReading): SoundCue[] {
  const cues: SoundCue[] = [];
  if (after.count !== (before?.count ?? null)) {
    if (BEEPED.includes(after.count)) cues.push("countdownBeep");
    if (after.count === "go") cues.push("go");
  }
  if (before === undefined) return cues;
  if (!before.airborne && after.airborne) cues.push("hop");
  if (!before.trick && after.trick) cues.push("trick");
  const charged = before.miniTurbo !== null && CHARGED.includes(before.miniTurbo);
  const boostBegan =
    after.boostSeconds !== null &&
    (before.boostSeconds === null || after.boostSeconds > before.boostSeconds);
  const rocketStarted =
    before.charge !== null && BOOSTING.includes(before.charge) && after.charge === null;
  const released = ((charged && after.miniTurbo === null) || rocketStarted) && boostBegan;
  const fired = before.item !== null && after.item === null;
  if (released) cues.push("miniTurbo");
  // A run beginning, or put back up by more than a correction could, is a
  // star's fire: once, whether the slot empties on the same frame, as in a
  // solo race, or only when the server confirms it, as in a room's, where
  // the prediction starts the run first. Its kick is no pad.
  const starRose =
    after.starSeconds !== null &&
    (before.starSeconds === null || after.starSeconds > before.starSeconds + STAR_RESTART_RISE_S);
  if (boostBegan && !released && !fired && !starRose) cues.push("boostPad");
  if (tierRank(after.miniTurbo) > tierRank(before.miniTurbo)) cues.push("tierChime");
  if (after.boxesTaken > before.boxesTaken) cues.push("itemPickup");
  if (before.reelStep !== null && after.reelStep !== null && after.reelStep > before.reelStep) {
    cues.push("itemRollTick");
  }
  if (before.rolling && !after.rolling && after.item !== null) cues.push("itemRolled");
  if (starRose) cues.push("starFire");
  else if (fired && before.item === "horn") cues.push("airHorn");
  else if (fired && before.item !== "star") cues.push("itemFire");
  if (after.coins > before.coins) cues.push("coinPickup");
  if (!before.spunOut && after.spunOut) cues.push("hit");
  if (after.coins < before.coins) cues.push("coinLoss");
  if (!before.stalled && after.stalled) cues.push("sputter");
  if (!before.wrongWay && after.wrongWay) cues.push("wrongWay");
  const finishedNow = !before.finished && after.finished;
  if (before.starSeconds !== null && after.starSeconds === null && !finishedNow) {
    cues.push("starEnd");
  }
  if (!before.towed && after.towed) cues.push("slipstream");
  if (after.flypastCuedMs !== null && after.flypastCuedMs !== before.flypastCuedMs) {
    cues.push("flypast");
  }
  if (!before.boxCall && after.boxCall) cues.push("boxChime");
  cues.push(...stopBeatCues(before.stop, after.stop));
  if (finishedNow) cues.push("finish");
  else if (after.lap > before.lap) cues.push("lap");
  return cues;
}

/** A step of the stop clock longer than this is a seek and sounds none of the beats it passes; a slow frame's step is well under it. */
const STOP_SEEK_S = 0.25;

/** The gun stall repeats this often while a botched stop's nut will not come. */
const GUN_STALL_EVERY_S = 1.3;
/** No stall starts within this long of the nut freeing, as on the pit crew page. */
const GUN_STALL_LAST_S = 0.6;
/** The act's short run on a stuck nut lasts this long; a botched stop's ends as the nut frees. */
const WHEEL_GUN_ACT_S = 0.3;
/** A worn wheel thumps down this far into its beat, once pulled clear of the hub. */
const WHEEL_OFF_INTO_S = 0.12;
/** A fresh wheel clunks home this long before its beat ends, as it seats. */
const WHEEL_ON_BEFORE_S = 0.08;

interface StopCue {
  readonly at: number;
  readonly cue: SoundCue;
}

/**
 * The cues of `stop`'s beats on the mark, by the stop clock, in order: the
 * jacks up, the guns off, a wheel off and on, the guns on, and the sign's
 * spin before green; then the tier's act from the same table as the
 * crew's poses: a slow stop's gun on the stubborn nut, a botched one's
 * stalls, the swarm's boots and the gun as the nut frees, a disaster's
 * failed jack, its rejack and the corner's redo.
 */
function stopCues(stop: HeardStop): StopCue[] {
  const beats = beatsOf(stop.stationaryS);
  const cues: StopCue[] = [
    { at: beats.jackUp[0], cue: "jackLift" },
    { at: beats.gunsOff[0], cue: "wheelGun" },
    { at: beats.wheelsOff[0] + WHEEL_OFF_INTO_S, cue: "wheelOff" },
    { at: beats.wheelsOn[1] - WHEEL_ON_BEFORE_S, cue: "wheelOn" },
    { at: beats.gunsOn[0], cue: "wheelGunOn" },
    { at: stop.stationaryS + RELEASE.spin[0], cue: "lollipopSpin" },
  ];
  const act = actOf(stop.tier, stop.stationaryS, stop.actFromS);
  if (act?.kind === "slow") cues.push({ at: act.stall[0], cue: "wheelGunAct" });
  if (act?.kind === "botched") {
    const [stuck, freed] = act.stall;
    for (let at = stuck; at < freed - GUN_STALL_LAST_S; at += GUN_STALL_EVERY_S) {
      cues.push({ at, cue: "gunStall" });
    }
    cues.push(
      { at: act.swarmAt, cue: "boots" },
      { at: freed - WHEEL_GUN_ACT_S, cue: "wheelGunAct" },
    );
  }
  if (act?.kind === "disaster") {
    const redo = act.redo[0];
    cues.push(
      { at: act.failAt, cue: "jackFail" },
      { at: act.rejack[0], cue: "jackLift" },
      { at: redo, cue: "wheelGunRedo" },
      { at: redo + REDO_STEPS_S[2] - WHEEL_ON_BEFORE_S, cue: "wheelOn" },
      { at: redo + REDO_STEPS_S[2], cue: "wheelGunRedo" },
    );
  }
  return cues.sort((a, b) => a.at - b.at);
}

/**
 * The stop's cues from `before` to `after`, the own kart's stop at each:
 * the boots as it is caught onto the guide, each beat the clock steps
 * across, and the jack's drop and the release chime as it is let go on
 * green. A seek past a beat or past green, or a stop first read on its
 * mark, sounds none.
 */
function stopBeatCues(before: HeardStop | null, after: HeardStop | null): SoundCue[] {
  if (before === null && after !== null) return after.clock < 0 ? ["boots"] : [];
  if (before !== null && after === null) {
    return before.stationaryS - before.clock <= STOP_SEEK_S ? ["jackDrop", "releaseChime"] : [];
  }
  if (before === null || after === null) return [];
  const step = after.clock - before.clock;
  if (step <= 0 || step > STOP_SEEK_S || after.stationaryS !== before.stationaryS) return [];
  return stopCues(after)
    .filter(({ at }) => before.clock < at && at <= after.clock)
    .map(({ cue }) => cue);
}

/**
 * The reading the next frame's cues compare against, after `after`: itself,
 * but for a stop clock a correction steps back, which holds at the furthest
 * `before` reached, so a beat already heard does not sound again as the
 * clock comes forward over it.
 */
export function heardOnward(before: SoundReading | undefined, after: SoundReading): SoundReading {
  const held = before?.stop;
  const stop = after.stop;
  if (held == null || stop === null || stop.stationaryS !== held.stationaryS) return after;
  return stop.clock < held.clock ? { ...after, stop: { ...stop, clock: held.clock } } : after;
}

/**
 * The other karts' blasts `after` carries that `before` did not: each is
 * heard once, the frame it first appears, however long the run reports
 * it.
 */
export function hornBlasts(before: SoundReading | undefined, after: SoundReading): HeardBlast[] {
  const heard = new Set(before?.blasts.map((blast) => blast.id));
  return after.blasts.filter((blast) => !heard.has(blast.id));
}

/** The warning's beep repeats this often at the warning's edge, and this often at the kart. */
const DRONE_BEEP_GAP_FAR_S = 0.35;
const DRONE_BEEP_GAP_NEAR_S = 0.1;
/** The rate the beep's file plays at by the time the drone reaches the kart; 1 at the warning's edge. */
const DRONE_BEEP_RATE_NEAR = 1.5;

/** How the drone warning's beep repeats: the gap to the next beep and the rate the file plays at. */
export interface DroneBeep {
  readonly gapS: number;
  readonly rate: number;
}

/**
 * The beep the warning calls for with the nearest drone chasing the own
 * kart `metres` away: none beyond the warning's reach; from there the gap
 * closes and the rate rises in a straight line from the edge's values to
 * the kart's as the drone reaches the kart.
 */
export function droneBeep(metres: number | null): DroneBeep | null {
  if (metres === null || !droneWarned(metres)) return null;
  const left = Math.max(metres, 0) / DRONE_WARNING_M;
  return {
    gapS: DRONE_BEEP_GAP_NEAR_S + (DRONE_BEEP_GAP_FAR_S - DRONE_BEEP_GAP_NEAR_S) * left,
    rate: DRONE_BEEP_RATE_NEAR - (DRONE_BEEP_RATE_NEAR - 1) * left,
  };
}

/** Within this far a sound from another kart plays at full level, falling in a straight line to silent at the far distance. */
export const SOUND_NEAR_M = 10;
export const SOUND_FAR_M = 60;
/** Another kart's sounds come through this lowpass, so they sit behind the own kart's. */
export const SOUND_AWAY_LOWPASS_HZ = 1200;

/**
 * The level, 0 to 1, a sound from a kart `metres` away plays at: full
 * within the near distance, a straight line to silent at the far one.
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
  if (reading.presentation.ambience.harbour > 0) loops.add("harbour");
  if (reading.presentation.crowd > 0) loops.add("crowd");
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
const LOOP_CURVES: Readonly<Record<SpeedLoop, LoopCurve>> = {
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
export function loopPlayback(loop: SpeedLoop, speed: number, topSpeed: number): LoopPlayback {
  const curve = LOOP_CURVES[loop];
  const rate = curve.centreHz(Math.abs(speed)) / curve.centreHz(curve.renderedSpeed);
  const fraction = (at: number) => Math.min(Math.abs(at) / topSpeed, LOOP_MAX_SPEED_FRACTION);
  return {
    rate: Math.min(Math.max(rate, LOOP_RATE_MIN), LOOP_RATE_MAX),
    gain: curve.swell(fraction(speed)) / curve.swell(fraction(curve.renderedSpeed)),
  };
}

/** Each ambience loop's gain at a zone's whole level. Presentation, so not the sim's to tune. */
const AMBIENCE_GAINS: Readonly<Record<AmbienceLoop, number>> = {
  harbour: 0.35,
  crowd: 0.3,
};

/** How `loop` plays in `blend`: never pitched, its gain the zone's level for it. */
export function ambiencePlayback(loop: AmbienceLoop, blend: PresentationBlend): LoopPlayback {
  const level = loop === "harbour" ? blend.ambience.harbour : blend.crowd;
  return { rate: 1, gain: AMBIENCE_GAINS[loop] * level };
}

/** Whether `loop` follows a zone rather than the kart's speed. */
export function isAmbience(loop: SoundLoop): loop is AmbienceLoop {
  return loop === "harbour" || loop === "crowd";
}
