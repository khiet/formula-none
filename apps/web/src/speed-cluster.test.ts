import { describe, expect, it } from "vitest";
import {
  drivetrain,
  shiftLights,
  tyreTopSpeed,
  tyreWearBand,
  tyreWearPercent,
} from "./speed-cluster";

// The sim's tuned top speed: 108 km/h, six bands of 18 km/h, 5 m/s each.
const TOP_SPEED = 30;
// The sim's tuned tyres: a set of ten, each worth 1% of top speed.
const PER_TYRE = 0.01;
const TYRE_SET = 10;

describe("the gear", () => {
  it("splits top speed into six even bands, changing up on each band's edge", () => {
    expect(drivetrain(1, TOP_SPEED).gear).toBe("1");
    expect(drivetrain(4.99, TOP_SPEED).gear).toBe("1");
    expect(drivetrain(5, TOP_SPEED).gear).toBe("2");
    expect(drivetrain(24.99, TOP_SPEED).gear).toBe("5");
    expect(drivetrain(25, TOP_SPEED).gear).toBe("6");
    expect(drivetrain(TOP_SPEED, TOP_SPEED).gear).toBe("6");
  });

  it("revs from 4,000 at a band's foot to 11,500 at its edge, and drops on the upshift", () => {
    expect(drivetrain(5, TOP_SPEED).rpm).toBe(4000);
    expect(drivetrain(7.5, TOP_SPEED).rpm).toBe(7750);
    expect(drivetrain(4.99, TOP_SPEED).rpm).toBeCloseTo(11485);
    expect(drivetrain(TOP_SPEED, TOP_SPEED).rpm).toBe(11500);
  });

  it("reads N while the speed shows as 0 km/h, either way", () => {
    expect(drivetrain(0, TOP_SPEED).gear).toBe("N");
    expect(drivetrain(0.1, TOP_SPEED).gear).toBe("N");
    expect(drivetrain(-0.1, TOP_SPEED).gear).toBe("N");
    expect(drivetrain(0, TOP_SPEED).rpm).toBe(4000);
  });

  it("reads R while reversing", () => {
    expect(drivetrain(-0.2, TOP_SPEED).gear).toBe("R");
    expect(drivetrain(-8, TOP_SPEED).gear).toBe("R");
    expect(drivetrain(-8, TOP_SPEED).limiter).toBe(false);
  });

  it("holds 6th on the limiter once the speed shows over top speed, as off a boost", () => {
    expect(drivetrain(TOP_SPEED, TOP_SPEED).limiter).toBe(false);
    expect(drivetrain(30.1, TOP_SPEED).limiter).toBe(false);
    const boosted = drivetrain(42, TOP_SPEED);
    expect(boosted.gear).toBe("6");
    expect(boosted.limiter).toBe(true);
    expect(boosted.rpm).toBe(12000);
  });

  it("moves its bands with a retuned top speed", () => {
    expect(drivetrain(3.99, 24).gear).toBe("1");
    expect(drivetrain(4, 24).gear).toBe("2");
    expect(drivetrain(24, 24).limiter).toBe(false);
    expect(drivetrain(25, 24).limiter).toBe(true);
  });
});

describe("the gear on tyres", () => {
  it("stretches its bands to the top speed ten tyres raise, so 33 m/s tops 6th and 30 m/s falls short", () => {
    const raised = tyreTopSpeed(TOP_SPEED, TYRE_SET, PER_TYRE);
    const top = drivetrain(33, raised);
    expect(top.gear).toBe("6");
    expect(top.rpm).toBeCloseTo(11500);
    expect(top.limiter).toBe(false);
    expect(shiftLights(top)).not.toContain("off");
    const short = drivetrain(30, raised);
    expect(short.gear).toBe("6");
    expect(short.rpm).toBeLessThan(11500);
    expect(shiftLights(short)).toContain("off");
  });

  it("keeps the limiter for a boost past the raised top speed", () => {
    expect(drivetrain(36, tyreTopSpeed(TOP_SPEED, TYRE_SET, PER_TYRE)).limiter).toBe(true);
  });

  it("tops 6th at 30 m/s on no tyres, as without them, and 33 m/s is over it", () => {
    const bare = tyreTopSpeed(TOP_SPEED, 0, PER_TYRE);
    expect(drivetrain(30, bare)).toEqual(drivetrain(30, TOP_SPEED));
    expect(drivetrain(30, bare).rpm).toBe(11500);
    expect(drivetrain(33, bare).limiter).toBe(true);
  });
});

describe("the shift lights", () => {
  it("are all off at idle", () => {
    expect(shiftLights(drivetrain(0, TOP_SPEED))).toEqual(Array(12).fill("off"));
  });

  it("light green, then red, then blue in thirds as the revs reach the shift", () => {
    expect(shiftLights(drivetrain(TOP_SPEED, TOP_SPEED))).toEqual([
      ...Array(4).fill("green"),
      ...Array(4).fill("red"),
      ...Array(4).fill("blue"),
    ]);
    expect(shiftLights(drivetrain(8.5, TOP_SPEED))).toEqual([
      ...Array(4).fill("green"),
      ...Array(3).fill("red"),
      ...Array(5).fill("off"),
    ]);
  });

  it("all turn blue on the limiter", () => {
    expect(shiftLights(drivetrain(42, TOP_SPEED))).toEqual(Array(12).fill("blue"));
  });
});

describe("the tyre gauge's reading", () => {
  it("reads the tyres carried against a full set: ten is 100%, one is 10%", () => {
    expect(tyreWearPercent(0, TYRE_SET)).toBe(0);
    expect(tyreWearPercent(1, TYRE_SET)).toBe(10);
    expect(tyreWearPercent(7, TYRE_SET)).toBe(70);
    expect(tyreWearPercent(10, TYRE_SET)).toBe(100);
  });
});

describe("the tyre gauge's colour", () => {
  it("is green above 60%, yellow above 30% and red below", () => {
    expect(tyreWearBand(100)).toBe("good");
    expect(tyreWearBand(61)).toBe("good");
    expect(tyreWearBand(60)).toBe("warn");
    expect(tyreWearBand(31)).toBe("warn");
    expect(tyreWearBand(30)).toBe("bad");
    expect(tyreWearBand(0)).toBe("bad");
  });
});
