import { describe, expect, it } from "vitest";
import { formatLapTime, NO_LAP_TIME, ordinal, ordinalParts } from "./format";

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

describe("ordinals", () => {
  it("split into the numeral and its suffix so the panel can size them apart", () => {
    expect(ordinalParts(1)).toEqual({ numeral: "1", suffix: "st" });
    expect(ordinalParts(2)).toEqual({ numeral: "2", suffix: "nd" });
    expect(ordinalParts(3)).toEqual({ numeral: "3", suffix: "rd" });
    expect(ordinalParts(8)).toEqual({ numeral: "8", suffix: "th" });
    expect(ordinal(8)).toBe("8th");
  });
});
