import { describe, expect, it } from "vitest";
import type { KartReadout } from "./bridge";
import { HEAD_LEVEL, type HeadSway, MAX_ROLL_RAD, ROLL_RAD_PER_G, swayHead } from "./head-sway";
import { kartReadout, yawed } from "./kart-readout-fixture";

const TICK_S = 1 / 60;
const FRAME_S = TICK_S;
const G = 9.81;

interface Turn {
  readonly speed: number;
  /** Radians per second about +Y; positive turns left. */
  readonly yawRate: number;
  readonly spinOutS?: number | null;
  readonly stallS?: number | null;
}

interface Drive {
  readonly sway: HeadSway;
  readonly previous: KartReadout | undefined;
  readonly yaw: number;
  /** Seconds driven; the ticks run so far follow from it. */
  readonly t: number;
}

const START: Drive = { sway: HEAD_LEVEL, previous: undefined, yaw: 0, t: 0 };

/**
 * Drive `seconds` through a steady `turn` from `start`, the sim ticking at
 * 60 Hz and the head swayed once a frame at `frameHz`: a frame between two
 * ticks shows the last tick's readout again, as the Race Scene draws it.
 */
function drive(turn: Turn, seconds: number, start: Drive = START, frameHz = 60): Drive {
  let { sway, previous, yaw, t } = start;
  const frameS = 1 / frameHz;
  for (let frame = 0; frame < Math.round(seconds * frameHz); frame++) {
    const ticks = Math.floor((t + frameS) / TICK_S + 1e-9) - Math.floor(t / TICK_S + 1e-9);
    t += frameS;
    yaw += turn.yawRate * TICK_S * ticks;
    const current =
      ticks === 0 && previous !== undefined
        ? previous
        : kartReadout({
            speed: turn.speed,
            rotation: yawed(yaw),
            spinOutS: turn.spinOutS ?? null,
            stallS: turn.stallS ?? null,
          });
    sway = swayHead(sway, previous, current, frameS);
    previous = current;
  }
  return { sway, previous, yaw, t };
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
    let state = START;
    for (let frame = 0; frame < 9; frame++) {
      state = drive(turnAt(2), FRAME_S, state);
      expect(state.sway.roll).toBeLessThanOrEqual(roll);
      expect(state.sway.roll).toBeGreaterThanOrEqual(-2 * ROLL_RAD_PER_G);
      roll = state.sway.roll;
    }
    expect(Math.abs(roll)).toBeGreaterThan(0.9 * 2 * ROLL_RAD_PER_G);
  });

  it("leans the same through a turn whose heading crosses half a turn round", () => {
    let state = drive(turnAt(2), 0.5, { ...START, yaw: Math.PI - 1 });
    const settled = state.sway.roll;
    while (state.yaw < Math.PI + 0.5) {
      state = drive(turnAt(2), FRAME_S, state);
      expect(state.sway.roll).toBeCloseTo(settled, 3);
    }
  });

  it("eases back to level on a straight", () => {
    const leaning = drive(turnAt(2), 1);
    const easing = drive({ speed: 20, yawRate: 0 }, 0.05, leaning);
    expect(Math.abs(easing.sway.roll)).toBeLessThan(Math.abs(leaning.sway.roll));
    expect(Math.abs(easing.sway.roll)).toBeGreaterThan(0);
    expect(drive({ speed: 20, yawRate: 0 }, 0.5, easing).sway.roll).toBeCloseTo(0, 4);
  });

  it.each([45, 60, 90, 120, 144])(
    "leans as far, without a shake, on a %i Hz display over the 60 Hz tick",
    (frameHz) => {
      const lean = -2 * ROLL_RAD_PER_G;
      let state = drive(turnAt(2), 1, START, frameHz);
      for (let frame = 0; frame < frameHz / 2; frame++) {
        state = drive(turnAt(2), 1 / frameHz, state, frameHz);
        // A frame that repeats a tick still ripples the lean, by under a degree at 2 g.
        expect(Math.abs(state.sway.roll / lean - 1)).toBeLessThan(0.08);
      }
    },
  );

  it("eases back to level when the kart stops dead mid-turn", () => {
    const leaning = drive(turnAt(2), 1);
    expect(drive({ speed: 0, yawRate: 0 }, 0.5, leaning).sway.roll).toBeCloseTo(0, 4);
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
