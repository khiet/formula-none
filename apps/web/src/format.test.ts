import { describe, expect, it } from "vitest";
import {
  formatLapTime,
  formatRunningLapTime,
  NO_LAP_TIME,
  ordinal,
  ordinalParts,
  speedParts,
} from "./format";

describe("lap times", () => {
  it("print in the 16-bit notation at the sim's precision: minutes, seconds and thousandths", () => {
    expect(formatLapTime(27.304)).toBe(`0'27"304`);
    expect(formatLapTime(75.4567)).toBe(`1'15"457`);
    expect(formatLapTime(0)).toBe(`0'00"000`);
  });

  it("round up into the next second and minute rather than printing 60", () => {
    expect(formatLapTime(14.9996)).toBe(`0'15"000`);
    expect(formatLapTime(59.9995)).toBe(`1'00"000`);
  });

  it("stand in dashes of the same shape when there is no time yet", () => {
    expect(NO_LAP_TIME).toBe(`-'--"---`);
    expect(NO_LAP_TIME).toHaveLength(formatLapTime(0).length);
  });
});

describe("the running clock", () => {
  it("prints minutes, seconds and tenths, truncated so it never runs ahead of the thousandths", () => {
    expect(formatRunningLapTime(11.999)).toBe(`0'11"9`);
    expect(formatRunningLapTime(75.4567)).toBe(`1'15"4`);
    expect(formatRunningLapTime(0)).toBe(`0'00"0`);
  });

  it("agrees with the thousandths it would round to", () => {
    expect(formatRunningLapTime(2.3)).toBe(`0'02"3`);
    expect(formatRunningLapTime(14.9996)).toBe(`0'15"0`);
    expect(formatRunningLapTime(59.9995)).toBe(`1'00"0`);
  });
});

describe("ordinals", () => {
  it("split into the numeral and its suffix so the panel can size them apart", () => {
    expect(ordinalParts(1)).toEqual({ numeral: "1", suffix: "st" });
    expect(ordinalParts(2)).toEqual({ numeral: "2", suffix: "nd" });
    expect(ordinalParts(3)).toEqual({ numeral: "3", suffix: "rd" });
    expect(ordinalParts(8)).toEqual({ numeral: "8", suffix: "th" });
    expect(ordinal(8)).toBe("8th");
  });
});

describe("speed", () => {
  it("prints whole km/h from the sim's m/s, the number apart from the unit", () => {
    expect(speedParts(0)).toEqual({ value: "0", unit: "km/h" });
    expect(speedParts(23.6)).toEqual({ value: "85", unit: "km/h" });
    expect(speedParts(30)).toEqual({ value: "108", unit: "km/h" });
  });

  it("stays positive while reversing, with no minus zero creeping to a stop", () => {
    expect(speedParts(-8)).toEqual({ value: "29", unit: "km/h" });
    expect(speedParts(-0.01)).toEqual({ value: "0", unit: "km/h" });
  });
});
