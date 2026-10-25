import { describe, expect, it } from "vitest";
import {
  ENGINE,
  type EngineSample,
  type EngineState,
  engineBandEdges,
  engineStep,
  engineVoice,
  FULL_THROTTLE_SECONDS_TO_TOP,
} from "./audio-engine";

const TOP_SPEED = 30;
const FRAME = 1 / 60;
/** How far a frame at full throttle moves the speed, up or down. */
const FULL_THROTTLE_STEP = (FRAME * TOP_SPEED) / FULL_THROTTLE_SECONDS_TO_TOP;

/** `frames` speeds from `from`, one frame apart, at full throttle (1) or a full lift (-1). */
function ramp(from: number, frames: number, direction: 1 | -1): number[] {
  return Array.from({ length: frames }, (_, i) => from + direction * (i + 1) * FULL_THROTTLE_STEP);
}

/** Step the engine through `speeds`, one frame apart, from `from`. */
function drive(
  speeds: readonly number[],
  from: EngineState | undefined = undefined,
  charge: EngineSample["charge"] = null,
): EngineState {
  let state = from;
  let time = from?.time ?? 0;
  for (const speed of speeds) {
    time += FRAME;
    state = engineStep(state, { speed, charge, time }, TOP_SPEED);
  }
  if (state === undefined) throw new Error("no speeds");
  return state;
}

/** The engine held steady at `speed` long enough for the load to settle. */
function steady(speed: number, from?: EngineState): EngineState {
  return drive(Array<number>(60).fill(speed), from);
}

describe("engineBandEdges", () => {
  it("splits top speed into six geometric bands from 0 to top", () => {
    const edges = engineBandEdges(TOP_SPEED);
    expect(edges).toHaveLength(ENGINE.gears + 1);
    expect(edges[0]).toBe(0);
    expect(edges.at(-1)).toBe(TOP_SPEED);
    expect(edges.map((edge) => Number(edge.toFixed(1)))).toEqual([
      0, 1.8, 4.4, 8.0, 13.1, 20.1, 30,
    ]);
    // Each band is the band ratio times the one below it.
    for (let gear = 2; gear <= ENGINE.gears; gear++) {
      const width = (edges[gear] ?? 0) - (edges[gear - 1] ?? 0);
      const below = (edges[gear - 1] ?? 0) - (edges[gear - 2] ?? 0);
      expect(width / below).toBeCloseTo(ENGINE.bandRatio, 6);
    }
  });
});

describe("engineStep", () => {
  it("idles in first at a standstill and revs through the band as speed rises", () => {
    const still = steady(0);
    expect(still.gear).toBe(1);
    expect(still.revs).toBe(0);
    expect(still.atLimiter).toBe(false);
    const edges = engineBandEdges(TOP_SPEED);
    const low = steady(1, still);
    const high = steady(1.7, low);
    expect(low.gear).toBe(1);
    expect(high.gear).toBe(1);
    expect(low.revs).toBeGreaterThanOrEqual(ENGINE.dropRatio);
    expect(high.revs).toBeGreaterThan(low.revs);
    expect(steady(edges[1] ?? 0, high).revs).toBeCloseTo(1, 6);
    // Backing up revs the same as going forward.
    expect(steady(-1, still).revs).toBe(low.revs);
  });

  it("upshifts as speed crosses the band's top, dropping the revs and cutting the note once", () => {
    const edges = engineBandEdges(TOP_SPEED);
    const below = steady((edges[1] ?? 0) - 0.01);
    expect(below.gear).toBe(1);
    expect(below.shift).toBeNull();
    const shifted = drive([(edges[1] ?? 0) + 0.01], below);
    expect(shifted.gear).toBe(2);
    expect(shifted.shift).toBe("up");
    expect(shifted.revs).toBeCloseTo(ENGINE.dropRatio, 2);
    expect(shifted.revs).toBeLessThan(below.revs);
    // The cut is the shift's frame alone.
    const after = drive([(edges[1] ?? 0) + 0.02], shifted);
    expect(after.gear).toBe(2);
    expect(after.shift).toBeNull();
  });

  it("climbs through every gear to sixth at top speed", () => {
    const edges = engineBandEdges(TOP_SPEED);
    let state: EngineState | undefined;
    for (let gear = 1; gear <= ENGINE.gears; gear++) {
      const middle = ((edges[gear - 1] ?? 0) + (edges[gear] ?? 0)) / 2;
      state = steady(middle, state);
      expect(state.gear).toBe(gear);
    }
    const top = steady(TOP_SPEED, state);
    expect(top.gear).toBe(ENGINE.gears);
    expect(top.revs).toBeCloseTo(1, 6);
    expect(top.atLimiter).toBe(false);
  });

  it("holds the gear as speed dips below the band's floor, and downshifts only past the hysteresis margin", () => {
    const edges = engineBandEdges(TOP_SPEED);
    const floor = edges[2] ?? 0;
    const margin = ENGINE.hysteresis * ((edges[3] ?? 0) - floor);
    const inThird = steady(floor + 1);
    expect(inThird.gear).toBe(3);
    const dipped = drive([floor - margin / 2], inThird);
    expect(dipped.gear).toBe(3);
    expect(dipped.shift).toBeNull();
    // Revs cannot fall under the drop ratio while the gear holds below its floor.
    expect(dipped.revs).toBeCloseTo(ENGINE.dropRatio, 6);
    const down = drive([floor - margin * 1.01], dipped);
    expect(down.gear).toBe(2);
    expect(down.shift).toBe("down");
    expect(down.revs).toBeGreaterThan(dipped.revs);
    // From a standing start the same speed picks second outright: hysteresis is for the gear held.
    expect(steady(floor - margin / 2).gear).toBe(2);
  });

  it("sits sixth on the limiter past top speed, and comes off it as speed falls back", () => {
    const boosted = steady(TOP_SPEED * 1.3);
    expect(boosted.gear).toBe(ENGINE.gears);
    expect(boosted.revs).toBe(ENGINE.limiter);
    expect(boosted.atLimiter).toBe(true);
    // Just over top speed the revs climb past 1 and the chop runs.
    const over = steady(TOP_SPEED * 1.02, boosted);
    expect(over.revs).toBeGreaterThan(1);
    expect(over.revs).toBeLessThan(ENGINE.limiter);
    expect(over.atLimiter).toBe(true);
    const back = steady(TOP_SPEED * 0.98, over);
    expect(back.revs).toBeLessThan(1);
    expect(back.atLimiter).toBe(false);
    expect(back.gear).toBe(ENGINE.gears);
  });

  it("revs on the grid by the rocket start's tier, holding the top note on the limiter for a full boost and a stall", () => {
    const grid = (charge: EngineSample["charge"]) => drive([0, 0, 0], undefined, charge);
    expect(grid("clean").revs).toBe(0);
    expect(grid("clean").atLimiter).toBe(false);
    expect(grid("weakBoost").revs).toBeGreaterThan(0);
    expect(grid("weakBoost").revs).toBeLessThan(1);
    expect(grid("weakBoost").atLimiter).toBe(false);
    expect(grid("fullBoost").revs).toBe(1);
    expect(grid("fullBoost").atLimiter).toBe(true);
    expect(grid("stall").revs).toBe(1);
    expect(grid("stall").atLimiter).toBe(true);
    // Every tier revs in first; the grid is no shift.
    expect(grid("fullBoost").gear).toBe(1);
    expect(grid("fullBoost").shift).toBeNull();
  });

  it("lets the charge rev the engine only at a standstill: once the kart rolls, speed rules", () => {
    const rolling = drive([10], undefined, "fullBoost");
    expect(rolling.gear).toBe(4);
    expect(rolling.revs).toBeLessThan(1);
    expect(rolling.atLimiter).toBe(false);
    // Clean at Go: the charge goes and the engine idles from the speed alone.
    const go = drive([0], drive([0], undefined, "weakBoost"), null);
    expect(go.revs).toBe(0);
  });

  it("infers load from the speed delta: positive pulling away, negative lifting, settling to none held steady", () => {
    const still = steady(0);
    expect(still.load).toBe(0);
    // Full throttle from the sim: top speed in 2.5 s.
    const pullingAway = drive(ramp(0, 30, 1), still);
    expect(pullingAway.load).toBeGreaterThan(0.9);
    expect(pullingAway.load).toBeLessThanOrEqual(1);
    const held = steady(pullingAway.speed, pullingAway);
    expect(Math.abs(held.load)).toBeLessThan(0.01);
    const lifting = drive(ramp(held.speed, 30, -1), held);
    expect(lifting.load).toBeLessThan(-0.9);
    expect(lifting.load).toBeGreaterThanOrEqual(-1);
  });

  it("smooths the load over a few frames rather than jumping with one reading", () => {
    const still = steady(0);
    const oneFrame = drive(ramp(0, 1, 1), still);
    expect(oneFrame.load).toBeGreaterThan(0);
    expect(oneFrame.load).toBeLessThan(0.5);
  });

  it("keeps the load through a reading at the same time, and clamps a collision's drop to full overrun", () => {
    const fast = steady(20);
    const sameTime = engineStep(fast, { speed: 25, charge: null, time: fast.time }, TOP_SPEED);
    expect(sameTime.load).toBe(fast.load);
    const hit = drive([0], fast);
    expect(hit.load).toBeGreaterThanOrEqual(-1);
    expect(hit.load).toBeLessThan(-0.1);
  });

  it("takes the gear from the speed alone on the first reading, with no shift to hear", () => {
    const first = engineStep(undefined, { speed: 20, charge: null, time: 5 }, TOP_SPEED);
    expect(first.gear).toBe(5);
    expect(first.shift).toBeNull();
    expect(first.load).toBe(0);
  });
});

describe("engineVoice", () => {
  it("pitches the note from idle to top by the revs, with the sub an octave under and the limiter over top", () => {
    expect(engineVoice(steady(0)).noteHz).toBe(ENGINE.idleHz);
    const top = engineVoice(steady(TOP_SPEED));
    expect(top.noteHz).toBeCloseTo(ENGINE.topHz, 6);
    expect(top.subHz).toBeCloseTo(ENGINE.topHz / 2, 6);
    const half = engineVoice(drive([0], steady(1)));
    expect(half.noteHz).toBe(ENGINE.idleHz);
    const limited = engineVoice(steady(TOP_SPEED * 1.3));
    expect(limited.noteHz).toBeGreaterThan(ENGINE.topHz);
    expect(limited.limiterDepth).toBeGreaterThan(0);
    expect(top.limiterDepth).toBe(0);
  });

  it("opens the filter with the revs and further on throttle, closing it on the overrun", () => {
    const coasting = engineVoice(steady(10));
    const pulling = engineVoice(drive(ramp(10, 30, 1), steady(10)));
    const lifting = engineVoice(drive(ramp(15, 30, -1), steady(15)));
    expect(coasting.filterHz).toBeGreaterThan(ENGINE.filterClosedHz);
    expect(coasting.filterHz).toBeLessThan(ENGINE.filterOpenHz);
    expect(pulling.filterHz).toBeGreaterThan(coasting.filterHz);
    expect(lifting.filterHz).toBeLessThan(coasting.filterHz);
    expect(engineVoice(steady(0)).filterHz).toBe(ENGINE.filterClosedHz);
    expect(engineVoice(steady(TOP_SPEED)).filterHz).toBeLessThanOrEqual(ENGINE.filterOpenHz);
  });

  it("drops the level and crackles on the overrun, and crackles neither on throttle nor near a standstill", () => {
    const lifting = engineVoice(drive(ramp(15, 30, -1), steady(15)));
    expect(lifting.level).toBeLessThan(1);
    expect(lifting.level).toBeGreaterThanOrEqual(ENGINE.overrunLevel);
    expect(lifting.crackle).toBeGreaterThan(0);
    expect(engineVoice(steady(15)).level).toBe(1);
    expect(engineVoice(steady(15)).crackle).toBe(0);
    const stopping = engineVoice(drive([1, 0.5, 0, 0, 0], steady(2)));
    expect(stopping.crackle).toBe(0);
  });

  it("breathes the intake with the revs, its band riding over the note", () => {
    const idle = engineVoice(steady(0));
    const top = engineVoice(steady(TOP_SPEED));
    expect(idle.intake).toBeGreaterThan(0);
    expect(top.intake).toBeGreaterThan(idle.intake);
    expect(top.intake).toBeCloseTo(ENGINE.intake, 6);
    expect(top.intakeHz).toBeGreaterThan(top.noteHz);
  });
});
