import { describe, expect, it } from "vitest";
import { reelFrame, reelStrip } from "./item-reel";

const ROLL_S = 2.5;

/** The reel's frame at every 1/60 s of a roll, from the pickup to the landing. */
function framesOf(rollS: number) {
  return Array.from({ length: Math.round(rollS * 60) + 1 }, (_, tick) =>
    reelFrame(tick / 60, rollS),
  );
}

describe("reelFrame", () => {
  it("starts on the strip's first icon and steps on through the roll, never back", () => {
    const frames = framesOf(ROLL_S);
    expect(frames[0]).toEqual({ step: 0, position: 0 });
    for (let at = 1; at < frames.length; at++) {
      expect(frames[at]?.step).toBeGreaterThanOrEqual(frames[at - 1]?.step ?? 0);
    }
  });

  it("changes fast at first and slow at the end, so it starts a blur and ends readable", () => {
    const changes: number[] = [];
    for (let at = 0; at <= 2500; at++) {
      const before = reelFrame((at - 1) / 1000, ROLL_S).step;
      if (at > 0 && reelFrame(at / 1000, ROLL_S).step !== before) changes.push(at / 1000);
    }
    const gaps = changes.slice(1).map((time, index) => time - (changes[index] ?? 0));
    expect(gaps[0]).toBeLessThan(0.08);
    expect(gaps.at(-1)).toBeGreaterThan(0.2);
    expect(changes.length).toBeGreaterThan(10);
  });

  it("lands on its last step before the roll ends and overshoots it a little, back on it at the end", () => {
    const frames = framesOf(ROLL_S);
    const last = frames.at(-1);
    expect(last?.position).toBeCloseTo(last?.step ?? 0, 5);
    const settling = frames.filter((frame) => frame.step === last?.step);
    expect(settling.length).toBeGreaterThan(5);
    const overshoot = Math.max(...settling.map((frame) => frame.position - frame.step));
    expect(overshoot).toBeGreaterThan(0.05);
    expect(overshoot).toBeLessThan(0.2);
    // Past the roll, as while a room's client waits on the snapshot, it stays put.
    expect(reelFrame(ROLL_S + 1, ROLL_S)).toEqual(last);
  });

  it("scrolls between the icons as it steps, never ahead of the next", () => {
    for (const frame of framesOf(ROLL_S)) {
      expect(frame.position).toBeGreaterThanOrEqual(frame.step);
      expect(frame.position).toBeLessThan(frame.step + 1);
    }
  });
});

describe("reelStrip", () => {
  it.each(["boost", "hazard", "homing", "star"] as const)(
    "stops on %s, the item the roll lands",
    (item) => {
      const strip = reelStrip(item, ROLL_S);
      const { step } = reelFrame(ROLL_S, ROLL_S);
      expect(strip[step]).toBe(item);
      // One past the landing, for the overshoot to show.
      expect(strip).toHaveLength(step + 2);
      strip.slice(1).forEach((icon, index) => {
        expect(icon).not.toBe(strip[index]);
      });
    },
  );
});
