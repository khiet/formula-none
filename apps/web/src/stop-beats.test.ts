import { describe, expect, it } from "vitest";
import type { Stop, StopTier } from "./bridge";
import { actOf, BEATS_SET_FOR_S, beatsOf, driverLook, kartLift, kartOnJacks } from "./stop-beats";

const CLEAN_LONGEST = 3;

function onMark(stationarySoFarS: number, stationaryS: number, tier: StopTier = "clean"): Stop {
  return { tier, stationaryS, guideLeftS: 0, stationarySoFarS };
}

describe("the stop's beats", () => {
  it("scales the beats down for a record stop and holds them for a longer one, the jacks dropping on the clock to green", () => {
    const record = beatsOf(2);
    expect(record.wheelsOff[0]).toBeCloseTo((0.85 * 2) / BEATS_SET_FOR_S);
    expect(record.done[0]).toBeLessThan(record.jackDown[0]);
    expect(record.jackDown).toEqual([1.7, 1.95]);
    const slow = beatsOf(4);
    expect(slow.wheelsOff[0]).toBeCloseTo(0.85);
    expect(slow.jackDown[0]).toBeCloseTo(3.7);
  });

  it("starts each tier's act past a clean stop's longest, and has none for a clean stop", () => {
    expect(actOf("clean", 2.9, CLEAN_LONGEST)).toBeNull();
    expect(actOf("record", 2, CLEAN_LONGEST)).toBeNull();
    expect(actOf("slow", 3.8, CLEAN_LONGEST)).toEqual({ kind: "slow", stall: [3, 3.3] });
    expect(actOf("botched", 6, CLEAN_LONGEST)).toEqual({
      kind: "botched",
      stall: [3, 5.5],
      swarmAt: 3.25,
    });
    expect(actOf("disaster", 10.5, CLEAN_LONGEST)).toEqual({
      kind: "disaster",
      failAt: 3,
      rejack: [7.5, 7.9],
      redo: [8, 9.4],
    });
  });
});

describe("the kart on its jacks", () => {
  it("lifts the front a beat ahead of the rear, and drops both for green", () => {
    expect(kartOnJacks(null, CLEAN_LONGEST)).toEqual({ front: 0, rear: 0 });
    expect(kartOnJacks({ ...onMark(0, 2.65), guideLeftS: 0.2 }, CLEAN_LONGEST)).toEqual({
      front: 0,
      rear: 0,
    });
    const rising = kartOnJacks(onMark(0.2, 2.65), CLEAN_LONGEST);
    expect(rising.front).toBeGreaterThan(rising.rear);
    expect(kartOnJacks(onMark(1, 2.65), CLEAN_LONGEST)).toEqual({ front: 1, rear: 1 });
    expect(kartOnJacks(onMark(2.64, 2.65), CLEAN_LONGEST)).toEqual({ front: 0, rear: 0 });
  });

  it("drops the nose on a disaster and lifts it again on the rejack", () => {
    const dropped = kartOnJacks(onMark(5, 10.5, "disaster"), CLEAN_LONGEST);
    expect(dropped.front).toBe(0);
    expect(dropped.rear).toBe(1);
    expect(kartLift(onMark(5, 10.5, "disaster"), CLEAN_LONGEST, 1.8).pitch).toBeLessThan(0);
    expect(kartOnJacks(onMark(8, 10.5, "disaster"), CLEAN_LONGEST).front).toBe(1);
  });

  it("raises the kart's middle by the jacks' lift once both ends are up", () => {
    expect(kartLift(onMark(1, 2.65), CLEAN_LONGEST, 1.8)).toEqual({ liftM: 0.16, pitch: 0 });
    expect(kartLift(onMark(0.2, 2.65), CLEAN_LONGEST, 1.8).pitch).toBeGreaterThan(0);
  });
});

describe("the driver's eyes", () => {
  it("are on the sign while on the mark, and back on the road as it spins", () => {
    expect(driverLook(null)).toBe(0);
    expect(driverLook({ ...onMark(0, 2.65), guideLeftS: 0.3 })).toBe(0);
    expect(driverLook(onMark(1, 2.65))).toBe(1);
    expect(driverLook(onMark(2.6, 2.65))).toBe(0);
  });
});
