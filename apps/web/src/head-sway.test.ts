import { describe, expect, it } from "vitest";
import type { KartReadout } from "./bridge";
import { HEAD_LEVEL, type HeadSway, MAX_ROLL_RAD, ROLL_RAD_PER_G, swayHead } from "./head-sway";
import { kartReadout, yawed } from "./kart-readout-fixture";

const FRAME_S = 1 / 60;
const G = 9.81;

interface Turn {
  readonly speed: number;
  /** Radians per second about +Y; positive turns left. */
  readonly yawRate: number;
  readonly spinOutS?: number | null;
  readonly stallS?: number | null;
}

/**
 * Drive `seconds` of one tick a frame through a steady `turn`, from `start`,
 * carrying on from `yaw` with `previous` as the last frame's readout.
 */
function drive(
  turn: Turn,
  seconds: number,
  start: { sway: HeadSway; previous: KartReadout | undefined; yaw: number } = {
    sway: HEAD_LEVEL,
    previous: undefined,
    yaw: 0,
  },
) {
  let { sway, previous, yaw } = start;
  for (let t = 0; t < seconds - 1e-9; t += FRAME_S) {
    yaw += turn.yawRate * FRAME_S;
    const current = kartReadout({
      speed: turn.speed,
      rotation: yawed(yaw),
      // A moving kart moves, so each tick's readout is a new one.
      position: [0, 0, -turn.speed * t],
      spinOutS: turn.spinOutS ?? null,
      stallS: turn.stallS ?? null,
    });
    sway = swayHead(sway, previous, current, FRAME_S);
    previous = current;
  }
  return { sway, previous, yaw };
}

/** A steady turn pulling `g` of lateral G at 20 m/s. */
function turnAt(g: number): Turn {
  const speed = 20;
  return { speed, yawRate: (g * G) / speed };
}

describe("head sway", () => {
  it("rolls the head to the right in a left turn, and to the left in a right turn", () => {
    // Three's roll about the kart's +Z: positive tips the head to the kart's left.
    expect(drive(turnAt(1), 1).sway.roll).toBeLessThan(0);
    expect(drive(turnAt(-1), 1).sway.roll).toBeGreaterThan(0);
  });

  it("rolls further the harder the turn, up to the cap, and holds there", () => {
    const one = Math.abs(drive(turnAt(1), 1).sway.roll);
    const two = Math.abs(drive(turnAt(2), 1).sway.roll);
    expect(one).toBeCloseTo(ROLL_RAD_PER_G, 3);
    expect(two).toBeCloseTo(2 * ROLL_RAD_PER_G, 3);
    expect(Math.abs(drive(turnAt(10), 1).sway.roll)).toBeCloseTo(MAX_ROLL_RAD, 6);
    expect(Math.abs(drive(turnAt(20), 3).sway.roll)).toBeCloseTo(MAX_ROLL_RAD, 6);
  });

  it("settles on a new lean in about 0.15 s, without overshooting it", () => {
    let roll = 0;
    let state = { sway: HEAD_LEVEL, previous: undefined as KartReadout | undefined, yaw: 0 };
    for (let frame = 0; frame < 9; frame++) {
      state = drive(turnAt(2), FRAME_S, state);
      expect(state.sway.roll).toBeLessThanOrEqual(roll);
      expect(state.sway.roll).toBeGreaterThanOrEqual(-2 * ROLL_RAD_PER_G);
      roll = state.sway.roll;
    }
    expect(Math.abs(roll)).toBeGreaterThan(0.9 * 2 * ROLL_RAD_PER_G);
  });

  it("eases back to level on a straight", () => {
    const leaning = drive(turnAt(2), 1);
    const easing = drive({ speed: 20, yawRate: 0 }, 0.05, leaning);
    expect(Math.abs(easing.sway.roll)).toBeLessThan(Math.abs(leaning.sway.roll));
    expect(Math.abs(easing.sway.roll)).toBeGreaterThan(0);
    expect(drive({ speed: 20, yawRate: 0 }, 0.5, easing).sway.roll).toBeCloseTo(0, 4);
  });

  it("holds the lean between ticks, so a frame faster than the tick does not shake it", () => {
    const leaning = drive(turnAt(2), 1);
    // Two frames of one tick: the readout has not moved since the last frame.
    const unchanged = kartReadout({ ...leaning.previous });
    const held = swayHead(leaning.sway, leaning.previous, unchanged, FRAME_S / 2);
    expect(held.roll).toBeCloseTo(leaning.sway.roll, 6);
  });

  it.each([
    ["spin-out", { spinOutS: 1 }],
    ["stall", { stallS: 1 }],
  ] as const)("holds the head level through a %s, then eases back into the lean", (_, timer) => {
    const leaning = drive(turnAt(3), 1);
    const struck = drive({ ...turnAt(3), ...timer }, 0.5, leaning);
    expect(struck.sway.roll).toBe(0);
    const after = drive(turnAt(3), FRAME_S, struck);
    expect(after.sway.roll).toBeLessThan(0);
    expect(Math.abs(after.sway.roll)).toBeLessThan(Math.abs(leaning.sway.roll) / 2);
    expect(drive(turnAt(3), 1, after).sway.roll).toBeCloseTo(leaning.sway.roll, 3);
  });

  it("starts level, with no last frame to measure a turn from", () => {
    expect(swayHead(HEAD_LEVEL, undefined, kartReadout({ speed: 20 }), FRAME_S).roll).toBe(0);
  });
});
