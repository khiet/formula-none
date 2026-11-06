// The pit crew's choreography (ADR 0082), as the mockup in
// docs/mockups/pit-crew-animations.html has it: a pure function of the
// member, the stop they work and the stop clock, so a frame drawn at any
// rate, or a client that joins mid-stop, poses the crew the same. Each
// member's part is a list of keys, a place and a pose from a second on the
// stop clock, played by walking from wherever the last key had got them.
//
// Places are in the pit box's own terms: metres from the mark's middle,
// `x` toward the building and `z` toward the kart's nose, laid out round
// the sim's 1.2 by 1.8 m kart. A yaw of 0 faces the nose, a quarter turn
// faces the building.

import type { Stop, StopTier } from "./bridge";
import { CREW_PER_BOX } from "./crew-roster";
import {
  actOf,
  beatsOf,
  CREW_BACK_S,
  clamp01,
  GREEN_HOLD_S,
  REDO_STEPS_S,
  RELEASE,
  ramp,
} from "./stop-beats";

/** A member's job, by member index. */
export const CREW_ROLES = [
  "front jack",
  "rear jack",
  "front lane-side gunner",
  "front building-side gunner",
  "rear lane-side gunner",
  "rear building-side gunner",
  "lollipop",
] as const;
export const FRONT_JACK = 0;
export const REAR_JACK = 1;
export const FIRST_GUNNER = 2;
export const LOLLIPOP = 6;

/**
 * A body's joint angles, radians. The torso leans forward by `lean`; the
 * head nods down by `headPitch` and turns toward its left by `headYaw`;
 * a shoulder or hip swings its limb forward, `roll` splays an arm out to
 * its side, an elbow bends the forearm forward and a knee, negative, bends
 * the shin back.
 */
export interface Joints {
  readonly lean: number;
  readonly headPitch: number;
  readonly headYaw: number;
  readonly shoulderL: number;
  readonly rollL: number;
  readonly elbowL: number;
  readonly shoulderR: number;
  readonly rollR: number;
  readonly elbowR: number;
  readonly hipL: number;
  readonly kneeL: number;
  readonly hipR: number;
  readonly kneeR: number;
}

const STAND: Joints = {
  lean: 0,
  headPitch: 0,
  headYaw: 0,
  shoulderL: 0,
  rollL: -0.12,
  elbowL: 0.15,
  shoulderR: 0,
  rollR: 0.12,
  elbowR: 0.15,
  hipL: 0,
  kneeL: 0,
  hipR: 0,
  kneeR: 0,
};
const CROUCHED_LEGS = { hipL: 1.5, kneeL: -1.55, hipR: 1.5, kneeR: -1.55 };

export const POSES = {
  stand: STAND,
  crouch: {
    ...STAND,
    ...CROUCHED_LEGS,
    lean: 0.45,
    headPitch: -0.15,
    shoulderL: 0.95,
    rollL: -0.2,
    elbowL: 0.5,
    shoulderR: 0.95,
    rollR: 0.2,
    elbowR: 0.5,
  },
  pull: {
    ...STAND,
    ...CROUCHED_LEGS,
    lean: 0.2,
    shoulderL: 0.6,
    rollL: -0.2,
    elbowL: 1.3,
    shoulderR: 0.6,
    rollR: 0.2,
    elbowR: 1.3,
  },
  push: {
    ...STAND,
    ...CROUCHED_LEGS,
    lean: 0.55,
    headPitch: -0.2,
    shoulderL: 1.3,
    rollL: -0.2,
    elbowL: 0.2,
    shoulderR: 1.3,
    rollR: 0.2,
    elbowR: 0.2,
  },
  handUp: {
    ...STAND,
    ...CROUCHED_LEGS,
    lean: 0.3,
    headPitch: -0.3,
    shoulderL: 0.95,
    rollL: -0.2,
    elbowL: 0.5,
    shoulderR: 3,
    rollR: 0.25,
    elbowR: 0.1,
  },
  jack: {
    ...STAND,
    lean: 0.35,
    headPitch: -0.2,
    shoulderL: 0.95,
    rollL: -0.15,
    elbowL: 0.3,
    shoulderR: 0.95,
    rollR: 0.15,
    elbowR: 0.3,
    hipL: 0.15,
    kneeL: -0.2,
    hipR: 0.15,
    kneeR: -0.2,
  },
  jackPush: {
    ...STAND,
    lean: 0.75,
    headPitch: -0.3,
    shoulderL: 0.35,
    rollL: -0.15,
    elbowL: 0.1,
    shoulderR: 0.35,
    rollR: 0.15,
    elbowR: 0.1,
    hipL: 0.5,
    kneeL: -0.6,
    hipR: 0.5,
    kneeR: -0.6,
  },
  dismay: {
    ...STAND,
    lean: -0.15,
    headPitch: -0.3,
    shoulderL: 2.4,
    rollL: -0.8,
    elbowL: 0.9,
    shoulderR: 2.4,
    rollR: 0.8,
    elbowR: 0.9,
  },
  holdPole: { ...STAND, shoulderL: 0.1, shoulderR: 1.2, rollR: 0.15, elbowR: 1.1 },
  raisePole: { ...STAND, lean: -0.08, headPitch: -0.2, shoulderR: 2.9, rollR: 0.25, elbowR: 0.1 },
} as const satisfies Record<string, Joints>;
export type Pose = keyof typeof POSES;

/** The stop a crew works, from the catch until it is back in its row. */
export interface CrewWork {
  readonly tier: StopTier;
  /** The stop's rolled stationary time: green is at this on the stop clock. */
  readonly stationaryS: number;
  /** A clean stop's longest: the tier's act starts here. */
  readonly actFromS: number;
  /** The stop clock at the catch, the tick the kart is taken onto its guide: the crew runs out here. */
  readonly catchS: number;
}

/** The crew's work for `stop`, its kart's stop, or null for none; the guide onto the mark takes `guideSeconds`. */
export function crewWork(
  stop: Stop | null,
  cleanStopSeconds: number,
  guideSeconds: number,
): CrewWork | null {
  if (stop === null) return null;
  return {
    tier: stop.tier,
    stationaryS: stop.stationaryS,
    actFromS: cleanStopSeconds,
    catchS: -guideSeconds,
  };
}

/**
 * The stop clock for `stop`: negative through the guide onto the mark, the
 * stationary seconds on it. Past green the kart has no stop, so the caller
 * carries the clock on from the stationary time.
 */
export function stopClock(stop: Stop): number {
  return stop.guideLeftS > 0 ? -stop.guideLeftS : stop.stationarySoFarS;
}

/** The crew is done with a stop, and back in its row, this long after green. */
export const WORK_ENDS_S = GREEN_HOLD_S + CREW_BACK_S;

/** Where a member is and what they are doing, in the pit box's terms. */
export interface Stance {
  readonly x: number;
  readonly z: number;
  readonly yaw: number;
  /** The pose blended from, the pose blended to and how far, 0 to 1. */
  readonly from: Pose;
  readonly pose: Pose;
  readonly blend: number;
  /** The stride's phase, radians, while walking; null standing. */
  readonly stride: number | null;
  /** How hard the gun shakes the member's hands, 0 to 1. */
  readonly judder: number;
  /** Added to the pose's head turn and nod. */
  readonly headYaw: number;
  readonly headPitch: number;
}

/** A jack trolley's place: its pad faces along `yaw`, its lever toward its member. */
export interface Place {
  readonly x: number;
  readonly z: number;
  readonly yaw: number;
}

const ROW_X = 2.6;
const ROW_PITCH_M = 0.7;
/** The row's order from the nose end, by member. */
const ROW_SLOT = [0, 6, 3, 2, 4, 5, 1];
const FACE_LANE = -Math.PI / 2;
const HUB_Z = 0.52;
const GUNNER_X = 1.1;
const STAND_OFF_X = 1.6;
/** The lever's length: a jack member stands this far behind the pad. */
const LEVER_M = 1.05;
const FRONT_JACK_UNDER = { x: 0, z: 1.3 };
/** The rear jack goes in angled, so its member stands off the chase camera's line to the kart. */
const REAR_JACK_UNDER = { x: 0.25, z: -1.35, yaw: -0.8 };
const REAR_JACK_WAITS = { x: 1, z: -2 };
const HOLDER_AT = { x: 0.75, z: 2.9 };
const HOLDER_ASIDE = { x: 1.35, z: 2.9 };
/** Behind the kart's tail, clear of the rear jack, for crossing to the lane side. */
const BEHIND_TAIL_Z = -3.1;

const RUN_M_PER_S = 6.5;
const STEP_M_PER_S = 3;
/** A pose change takes this long, and so does turning to walk. */
const POSE_S = 0.15;
/** Radians of stride per metre walked. */
const STRIDE_RAD_PER_M = 3.4;
const WALK_BACK_S = 1.8;

interface Key {
  readonly at: number;
  readonly x: number;
  readonly z: number;
  readonly yaw: number;
  readonly pose?: Pose;
  readonly judder?: number;
  readonly headYaw?: number;
  readonly headPitch?: number;
  /** Arrive by this on the stop clock, whatever the distance; else at `speed`. */
  readonly by?: number;
  readonly speed?: number;
}

interface State {
  readonly x: number;
  readonly z: number;
  readonly yaw: number;
  readonly pose: Pose;
  readonly headYaw: number;
  readonly headPitch: number;
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
function turn(from: number, to: number, t: number): number {
  const delta = Math.atan2(Math.sin(to - from), Math.cos(to - from));
  return from + delta * t;
}

/** A key played from `start`, `dt` seconds after its time. */
function play(start: State, key: Key, dt: number): Stance {
  const dx = key.x - start.x;
  const dz = key.z - start.z;
  const distance = Math.hypot(dx, dz);
  const moved = distance > 0.05;
  const travel = moved
    ? Math.max(0.05, key.by !== undefined ? key.by - key.at : distance / (key.speed ?? RUN_M_PER_S))
    : 0;
  const along = travel > 0 ? clamp01(dt / travel) : 1;
  const x = lerp(start.x, key.x, along);
  const z = lerp(start.z, key.z, along);
  const pose = key.pose ?? start.pose;
  const headYaw = key.headYaw ?? 0;
  const headPitch = key.headPitch ?? 0;
  if (moved && along < 1) {
    return {
      x,
      z,
      yaw: turn(start.yaw, Math.atan2(dx, dz), ramp(dt, 0, POSE_S)),
      from: start.pose,
      pose: "stand",
      blend: ramp(dt, 0, POSE_S),
      stride: along * distance * STRIDE_RAD_PER_M,
      judder: 0,
      headYaw: lerp(start.headYaw, 0, ramp(dt, 0, POSE_S)),
      headPitch: lerp(start.headPitch, 0, ramp(dt, 0, POSE_S)),
    };
  }
  const settle = ramp(dt - travel, 0, POSE_S);
  return {
    x,
    z,
    yaw: turn(moved ? Math.atan2(dx, dz) : start.yaw, key.yaw, settle),
    from: moved ? "stand" : start.pose,
    pose,
    blend: settle,
    stride: null,
    judder: key.judder ?? 0,
    headYaw: lerp(moved ? 0 : start.headYaw, headYaw, settle),
    headPitch: lerp(moved ? 0 : start.headPitch, headPitch, settle),
  };
}

/** The member's stance `t` into a part of `keys`, the first a place to start from. */
function perform(keys: readonly Key[], t: number): Stance {
  const [first, ...rest] = keys;
  if (first === undefined) throw new RangeError("a part has a key");
  let start: State = {
    x: first.x,
    z: first.z,
    yaw: first.yaw,
    pose: first.pose ?? "stand",
    headYaw: first.headYaw ?? 0,
    headPitch: first.headPitch ?? 0,
  };
  let current: Key = first;
  for (const key of rest) {
    if (key.at > t) break;
    const reached = play(start, current, key.at - current.at);
    start = {
      x: reached.x,
      z: reached.z,
      yaw: reached.yaw,
      pose: reached.blend >= 0.5 ? reached.pose : reached.from,
      headYaw: reached.headYaw,
      headPitch: reached.headPitch,
    };
    current = key;
  }
  return play(start, current, t - current.at);
}

/** A key at `key`'s place from `at`, with no arrival time of its own. */
function still(key: Key, at: number, extra: Partial<Key>): Key {
  const { by: _by, ...place } = key;
  return { ...place, at, ...extra };
}

/** The stop clock at which a walk from `from` to `key` ends. */
function arrival(from: { readonly x: number; readonly z: number }, key: Key): number {
  return key.by ?? key.at + Math.hypot(key.x - from.x, key.z - from.z) / (key.speed ?? RUN_M_PER_S);
}

/** `key`, timed to start as the walk from `from` to `after` ends. */
function then(
  from: { readonly x: number; readonly z: number },
  after: Key,
  key: Omit<Key, "at">,
): Key {
  return { ...key, at: arrival(from, after) };
}

/** The walk back to the row once the hold after green is over. */
function walkBack(member: number, work: CrewWork): Key {
  const back = work.stationaryS + GREEN_HOLD_S;
  return { ...rowSpot(member), at: back, by: back + WALK_BACK_S };
}

function rowSpot(member: number): Key {
  const slot = ROW_SLOT[member] ?? member;
  return {
    at: Number.NEGATIVE_INFINITY,
    x: ROW_X,
    z: ((CREW_PER_BOX - 1) / 2 - slot) * ROW_PITCH_M,
    yaw: FACE_LANE,
    pose: "stand",
  };
}

/** Behind a jack's lever: where its member stands to work it. */
function behind(jack: Place): { x: number; z: number; yaw: number } {
  return {
    x: jack.x - Math.sin(jack.yaw) * LEVER_M,
    z: jack.z - Math.cos(jack.yaw) * LEVER_M,
    yaw: jack.yaw,
  };
}

/** The keys of a jack trolley: parked in front of its member in the row, rolled under its end, dragged clear. */
function jackKeys(end: "front" | "rear", work: CrewWork): Key[] {
  const member = end === "front" ? FRONT_JACK : REAR_JACK;
  const row = rowSpot(member);
  const parked = { at: row.at, x: row.x - LEVER_M, z: row.z, yaw: FACE_LANE };
  const green = work.stationaryS;
  const [dropFrom] = beatsOf(green).jackDown;
  // Dragged out from under the dropping kart, clear of its path just after green.
  const clear = { at: dropFrom - 0.15, by: green + 0.05 };
  const { at: backAt, by: backBy } = walkBack(member, work);
  const back = { at: backAt, by: backBy };
  if (end === "front") {
    return [
      parked,
      { at: work.catchS, by: -0.05, ...FRONT_JACK_UNDER, yaw: Math.PI },
      { ...clear, x: -0.9, z: 1.6, yaw: Math.PI },
      { ...back, x: parked.x, z: parked.z, yaw: parked.yaw },
    ];
  }
  return [
    parked,
    { at: work.catchS, by: -0.2, ...REAR_JACK_WAITS, yaw: 0 },
    { at: -0.15, by: 0.1, ...REAR_JACK_UNDER },
    { ...clear, ...REAR_JACK_UNDER, z: -1.75 },
    { ...back, x: parked.x, z: parked.z, yaw: parked.yaw },
  ];
}

/** Where a pit box's front or rear jack trolley is, `t` on the stop clock of `work`, or parked for no work. */
export function jackPlace(end: "front" | "rear", work: CrewWork | null, t: number): Place {
  const keys = jackKeys(end, work ?? { tier: "clean", stationaryS: 0, actFromS: 0, catchS: 0 });
  const { x, z, yaw } = perform(work === null ? keys.slice(0, 1) : keys, t);
  return { x, z, yaw };
}

function jackMemberKeys(end: "front" | "rear", work: CrewWork): Key[] {
  const member = end === "front" ? FRONT_JACK : REAR_JACK;
  const beats = beatsOf(work.stationaryS);
  const act = actOf(work.tier, work.stationaryS, work.actFromS);
  const green = work.stationaryS;
  const [upFrom, upTo] = beats.jackUp;
  const lag = end === "front" ? 0 : 0.05;
  const jack = jackKeys(end, work);
  const working = (key: Key, pose: Pose): Key => ({ ...key, ...behind(key), pose });
  const keys: Key[] = [rowSpot(member)];
  // Every move of the jack is the member's, a lever's length behind it.
  for (const key of jack.slice(1, -1)) keys.push(working(key, "jack"));
  const under = keys[keys.length - 2] as Key;
  keys.push(still(under, upFrom + lag, { pose: "jackPush" }));
  keys.push(still(under, upTo + lag, { pose: "jack" }));
  if (end === "front" && act?.kind === "disaster") {
    keys.push({
      at: act.failAt,
      x: 0.4,
      z: 3.3,
      yaw: Math.PI,
      pose: "dismay",
      speed: STEP_M_PER_S,
    });
    keys.push({ ...under, at: act.rejack[0], by: act.rejack[0] + 0.3, pose: "jackPush" });
    keys.push(still(under, act.rejack[1], { pose: "jack" }));
  }
  const dragged = working(jack[jack.length - 2] as Key, "jack");
  keys.push(still(dragged, green + 0.05, { pose: "stand", headYaw: end === "front" ? -0.6 : 0 }));
  keys.push(walkBack(member, work));
  return keys.sort((a, b) => a.at - b.at);
}

function gunnerKeys(corner: number, work: CrewWork): Key[] {
  const member = FIRST_GUNNER + corner;
  const side = corner % 2 === 0 ? -1 : 1;
  const end = corner < 2 ? 1 : -1;
  const hub = { x: side * GUNNER_X, z: end * HUB_Z, yaw: side > 0 ? -Math.PI / 2 : Math.PI / 2 };
  const beats = beatsOf(work.stationaryS);
  const act = actOf(work.tier, work.stationaryS, work.actFromS);
  const green = work.stationaryS;
  const at = (time: number, pose: Pose, judder = 0, place = hub): Key => ({
    at: time,
    ...place,
    pose,
    judder,
  });
  const row = rowSpot(member);
  const keys: Key[] = [row];
  // The rear lane-side gunner goes round the nose, ahead of the kart on its guide.
  const run = { ...hub, pose: "crouch" as const };
  if (corner === 2) {
    const round = { at: work.catchS, x: 0, z: 1.6, yaw: 0, pose: "stand" as const };
    keys.push(round, then(row, round, run));
  } else {
    keys.push({ at: work.catchS, ...run });
  }
  keys.push(
    at(beats.gunsOff[0], "crouch", 1),
    at(beats.wheelsOff[0], "pull"),
    at(beats.wheelsOn[0], "push"),
    at(beats.gunsOn[0], "crouch", 1),
    at(beats.done[0], "handUp"),
  );
  let standOff = { x: side * STAND_OFF_X, z: hub.z, yaw: hub.yaw };
  const stuck = corner === 2;
  if ((act?.kind === "slow" || act?.kind === "botched") && stuck) {
    keys.push(at(act.stall[0], "crouch", 1), at(act.stall[1], "handUp"));
  } else if (act?.kind === "botched") {
    // The other three crowd the stuck wheel, the building side's round the tail.
    const spots = [
      { x: -GUNNER_X, z: -HUB_Z - 0.5, yaw: Math.PI / 2 + 0.6 },
      { x: -GUNNER_X - 0.55, z: -HUB_Z, yaw: Math.PI / 2 },
      { x: -GUNNER_X, z: -HUB_Z + 0.5, yaw: Math.PI / 2 - 0.6 },
    ];
    const spot = spots[corner > 2 ? corner - 1 : corner] ?? spots[0];
    if (spot === undefined) throw new RangeError("a spot per gunner");
    const pose: Pose = corner === 3 ? "crouch" : "push";
    const judder = corner === 3 ? 0.7 : 0;
    let crowd: Key = { at: act.swarmAt, ...spot, pose, judder };
    if (side > 0) {
      const outside: Key = {
        at: act.swarmAt,
        x: STAND_OFF_X,
        z: BEHIND_TAIL_Z,
        yaw: Math.PI,
        pose: "stand",
      };
      const across = then(hub, outside, {
        x: -STAND_OFF_X,
        z: BEHIND_TAIL_Z,
        yaw: 0,
        pose: "stand",
      });
      crowd = then(outside, across, crowd);
      keys.push(outside, across);
    }
    keys.push(crowd, { ...crowd, at: Math.max(crowd.at, act.stall[1]), pose: "handUp", judder: 0 });
    standOff = { x: spot.x - 0.5, z: spot.z, yaw: spot.yaw };
  } else if (act?.kind === "disaster") {
    keys.push({
      at: act.failAt,
      x: side * STAND_OFF_X,
      z: hub.z,
      yaw: hub.yaw,
      pose: "stand",
      headYaw: side > 0 ? 0.9 : -0.9,
      speed: STEP_M_PER_S,
    });
    if (corner < 2) {
      const [redoFrom, redoTo] = act.redo;
      const [off, pulled, pushed] = REDO_STEPS_S;
      keys.push(
        at(act.rejack[0], "crouch"),
        at(redoFrom, "crouch", 1),
        at(redoFrom + off, "pull"),
        at(redoFrom + pulled, "push"),
        at(redoFrom + pushed, "crouch", 1),
        at(redoTo, "handUp"),
      );
    } else {
      keys.push(at(act.rejack[0], "handUp"));
    }
  }
  keys.sort((a, b) => a.at - b.at);
  keys.push({
    at: green,
    ...standOff,
    pose: "stand",
    headYaw: standOff.x > 0 ? 0.9 : -0.9,
    speed: STEP_M_PER_S,
  });
  keys.push(walkBack(member, work));
  return keys;
}

function holderKeys(work: CrewWork): Key[] {
  const green = work.stationaryS;
  const [checkFrom, checkTo] = RELEASE.laneCheck;
  const holding = { ...HOLDER_AT, yaw: Math.PI, pose: "holdPole" as const, headPitch: 0.1 };
  const row = rowSpot(LOLLIPOP);
  const keys: Key[] = [
    row,
    { at: work.catchS, ...holding },
    { at: green + checkFrom, ...holding, headYaw: 0.8, headPitch: -0.1 },
    { at: green + checkTo, ...holding },
    {
      at: green + RELEASE.stepAside,
      ...HOLDER_ASIDE,
      yaw: Math.PI + 0.3,
      pose: "raisePole",
      headYaw: -0.4,
      speed: STEP_M_PER_S,
    },
    walkBack(LOLLIPOP, work),
  ];
  return keys.sort((a, b) => a.at - b.at);
}

/**
 * Where crew member `member` stands and how, `secondsOnMark` on the stop
 * clock of `work`, or in the row with no work. From the catch the crew
 * runs out: the jacks to each end, a gunner to every hub, the holder off
 * the nose. On the mark the gunners work their beats, the tier acts its
 * roll out past a clean stop's longest, and on green the holder has
 * stepped aside, the jacks drag clear and the gunners stand off, until the
 * hold is over and they walk back.
 */
export function stance(member: number, work: CrewWork | null, secondsOnMark: number): Stance {
  if (work === null) return perform([rowSpot(member)], secondsOnMark);
  if (member === FRONT_JACK) return perform(jackMemberKeys("front", work), secondsOnMark);
  if (member === REAR_JACK) return perform(jackMemberKeys("rear", work), secondsOnMark);
  if (member === LOLLIPOP) return perform(holderKeys(work), secondsOnMark);
  return perform(gunnerKeys(member - FIRST_GUNNER, work), secondsOnMark);
}

/** The lollipop's sign: how far it has spun from STOP to GO, and how far it has lifted, each 0 to 1. */
export function lollipopSign(
  work: CrewWork | null,
  secondsOnMark: number,
): { readonly spun: number; readonly lifted: number } {
  if (work === null) return { spun: 0, lifted: 0 };
  const green = work.stationaryS;
  const back = green + GREEN_HOLD_S;
  const down = 1 - ramp(secondsOnMark, back, back + 0.3);
  return {
    spun: ramp(secondsOnMark, green + RELEASE.spin[0], green + RELEASE.spin[1]) * down,
    lifted: ramp(secondsOnMark, green + RELEASE.lift[0], green + RELEASE.lift[1]) * down,
  };
}

/** A pose's joints, `blend` of the way from `from` to `pose`. */
export function jointsOf(from: Pose, pose: Pose, blend: number): Joints {
  const a = POSES[from];
  const b = POSES[pose];
  const out = {} as Record<keyof Joints, number>;
  for (const joint of Object.keys(a) as (keyof Joints)[]) {
    out[joint] = lerp(a[joint], b[joint], blend);
  }
  return out;
}
