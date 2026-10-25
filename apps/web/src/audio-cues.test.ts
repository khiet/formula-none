import { describe, expect, it } from "vitest";
import {
  enginePitchHz,
  loopPlayback,
  type SoundReading,
  soundCues,
  soundLoops,
  soundReading,
} from "./audio-cues";
import type { KartTransform, Phase, Standing } from "./bridge";

const STILL: SoundReading = {
  speed: 0,
  airborne: false,
  miniTurbo: null,
  item: null,
  spunOut: false,
  boostSeconds: null,
  lap: 1,
  finished: false,
  count: "",
  charge: null,
  stalled: false,
  surface: "asphalt",
  drifting: false,
  wrongWay: false,
};

describe("soundCues", () => {
  it("is silent with no change, and before there is a reading to change from", () => {
    expect(soundCues(STILL, STILL)).toEqual([]);
    expect(soundCues(undefined, { ...STILL, airborne: true, item: "boost" })).toEqual([]);
  });

  it("beeps as the count the panel shows changes to 3, 2 and 1, and sounds GO as it shows GO", () => {
    const at = (count: string): SoundReading => ({ ...STILL, count });
    // The 3 is up from the first reading, so it is heard from nothing.
    expect(soundCues(undefined, at("3"))).toEqual(["countdownBeep"]);
    expect(soundCues(at("3"), at("3"))).toEqual([]);
    expect(soundCues(at("3"), at("2"))).toEqual(["countdownBeep"]);
    expect(soundCues(at("2"), at("1"))).toEqual(["countdownBeep"]);
    expect(soundCues(at("1"), at("GO"))).toEqual(["go"]);
    expect(soundCues(at("GO"), at("GO"))).toEqual([]);
    expect(soundCues(at("GO"), at(""))).toEqual([]);
    expect(soundCues(at(""), at(""))).toEqual([]);
  });

  it("plays the hop as the kart leaves the ground, not while it is up or as it lands", () => {
    const up = { ...STILL, airborne: true };
    expect(soundCues(STILL, up)).toEqual(["hop"]);
    expect(soundCues(up, up)).toEqual([]);
    expect(soundCues(up, STILL)).toEqual([]);
  });

  it("plays the mini-turbo when a charged drift ends into its boost, and nothing for an uncharged one", () => {
    const boosting = { ...STILL, boostSeconds: 0.6 };
    expect(soundCues({ ...STILL, miniTurbo: "blue" }, boosting)).toEqual(["miniTurbo"]);
    expect(soundCues({ ...STILL, miniTurbo: "orange" }, boosting)).toEqual(["miniTurbo"]);
    expect(soundCues({ ...STILL, miniTurbo: "none" }, STILL)).toEqual([]);
    // A boost from a pad with no drift ending is not a release either.
    expect(soundCues(STILL, boosting)).toEqual(["boostPad"]);
    // Let go while a pad's boost still runs: the boost lengthens, so it is heard.
    expect(
      soundCues(
        { ...STILL, miniTurbo: "blue", boostSeconds: 0.2 },
        { ...STILL, boostSeconds: 0.6 },
      ),
    ).toEqual(["miniTurbo"]);
    // Charging through the tiers is the chime's tell, not the release's.
    expect(soundCues({ ...STILL, miniTurbo: "none" }, { ...STILL, miniTurbo: "blue" })).toEqual([
      "tierChime",
    ]);
  });

  it("plays the tier chime as the drift charges to blue and again to orange, the same chime both times", () => {
    const at = (miniTurbo: SoundReading["miniTurbo"]): SoundReading => ({
      ...STILL,
      miniTurbo,
      drifting: miniTurbo !== null,
    });
    expect(soundCues(STILL, at("none"))).toEqual([]);
    expect(soundCues(at("none"), at("none"))).toEqual([]);
    expect(soundCues(at("none"), at("blue"))).toEqual(["tierChime"]);
    expect(soundCues(at("blue"), at("blue"))).toEqual([]);
    expect(soundCues(at("blue"), at("orange"))).toEqual(["tierChime"]);
    expect(soundCues(at("orange"), at("orange"))).toEqual([]);
    // The drift ending, charged or not, is no chime.
    expect(soundCues(at("orange"), STILL)).toEqual([]);
    expect(soundCues(at("blue"), { ...STILL, boostSeconds: 0.6 })).toEqual(["miniTurbo"]);
  });

  it("plays the boost pad as a boost starts or lengthens that no release and no item fired explains", () => {
    const boosting = { ...STILL, boostSeconds: 0.6 };
    expect(soundCues(STILL, boosting)).toEqual(["boostPad"]);
    expect(soundCues(boosting, boosting)).toEqual([]);
    // A boost ticking down is the same boost; a second pad lengthens it.
    expect(soundCues(boosting, { ...STILL, boostSeconds: 0.5 })).toEqual([]);
    expect(soundCues({ ...STILL, boostSeconds: 0.2 }, boosting)).toEqual(["boostPad"]);
    // A charged drift's release is the mini-turbo, not a pad.
    expect(soundCues({ ...STILL, miniTurbo: "orange", drifting: true }, boosting)).toEqual([
      "miniTurbo",
    ]);
    // An uncharged drift ending onto a pad is a pad.
    expect(soundCues({ ...STILL, miniTurbo: "none", drifting: true }, boosting)).toEqual([
      "boostPad",
    ]);
    // A boost item fired is the fire, not a pad.
    expect(soundCues({ ...STILL, item: "boost" }, boosting)).toEqual(["itemFire"]);
    // The rocket start's boost at GO is the mini-turbo.
    expect(
      soundCues({ ...STILL, count: "1", charge: "fullBoost" }, { ...boosting, count: "GO" }),
    ).toEqual(["go", "miniTurbo"]);
  });

  it("plays the wrong way once as the flag raises and nothing while it holds or drops", () => {
    const wrong = { ...STILL, wrongWay: true };
    expect(soundCues(STILL, wrong)).toEqual(["wrongWay"]);
    expect(soundCues(wrong, wrong)).toEqual([]);
    expect(soundCues(wrong, STILL)).toEqual([]);
  });

  it("plays no mini-turbo for a charged drift that ends without its boost", () => {
    // Spun out mid-charge: the hit, and only the hit.
    expect(soundCues({ ...STILL, miniTurbo: "orange" }, { ...STILL, spunOut: true })).toEqual([
      "hit",
    ]);
    // Slowed below drift speed, or respawned: the drift just ends.
    expect(soundCues({ ...STILL, miniTurbo: "blue" }, STILL)).toEqual([]);
  });

  it("plays the mini-turbo as a rocket start's charge becomes its boost at GO", () => {
    const charging = (charge: SoundReading["charge"]): SoundReading => ({
      ...STILL,
      count: "1",
      charge,
    });
    const go = { ...STILL, count: "GO" };
    expect(soundCues(charging("fullBoost"), { ...go, boostSeconds: 1.5 })).toEqual([
      "go",
      "miniTurbo",
    ]);
    expect(soundCues(charging("weakBoost"), { ...go, boostSeconds: 0.6 })).toEqual([
      "go",
      "miniTurbo",
    ]);
    // A clean start has no boost to hear, and the charge's tiers are the sparks' tell.
    expect(soundCues(charging("clean"), go)).toEqual(["go"]);
    expect(soundCues(charging("weakBoost"), charging("fullBoost"))).toEqual([]);
  });

  it("plays the sputter as a stall begins, once, and no mini-turbo for the hold that stalled", () => {
    const stalled = { ...STILL, count: "GO", stalled: true };
    expect(soundCues({ ...STILL, count: "1", charge: "stall" }, stalled)).toEqual([
      "go",
      "sputter",
    ]);
    expect(soundCues(stalled, stalled)).toEqual([]);
    expect(soundCues(stalled, { ...STILL, count: "GO" })).toEqual([]);
  });

  it("plays the pickup as the slot fills and the fire as it empties", () => {
    const held = { ...STILL, item: "hazard" as const };
    expect(soundCues(STILL, held)).toEqual(["itemPickup"]);
    expect(soundCues(held, held)).toEqual([]);
    expect(soundCues(held, STILL)).toEqual(["itemFire"]);
  });

  it("plays the hit as the spin-out begins, once", () => {
    const spun = { ...STILL, spunOut: true };
    expect(soundCues(STILL, spun)).toEqual(["hit"]);
    expect(soundCues(spun, spun)).toEqual([]);
    expect(soundCues(spun, STILL)).toEqual([]);
  });

  it("plays the lap as the lap count rises and the finish in its place as the kart finishes", () => {
    expect(soundCues(STILL, { ...STILL, lap: 2 })).toEqual(["lap"]);
    expect(soundCues({ ...STILL, lap: 3 }, { ...STILL, lap: 3, finished: true })).toEqual([
      "finish",
    ]);
    expect(soundCues({ ...STILL, lap: 3 }, { ...STILL, lap: 4, finished: true })).toEqual([
      "finish",
    ]);
    expect(
      soundCues({ ...STILL, lap: 3, finished: true }, { ...STILL, lap: 3, finished: true }),
    ).toEqual([]);
  });

  it("reports every change of one frame together", () => {
    const after = { ...STILL, airborne: true, item: "boost" as const, lap: 2 };
    expect(soundCues(STILL, after)).toEqual(["hop", "itemPickup", "lap"]);
  });
});

describe("soundLoops", () => {
  it("runs the drift skid while the kart drifts, at any tier", () => {
    expect(soundLoops({ ...STILL, drifting: true, miniTurbo: "none" })).toEqual(
      new Set(["driftSkid"]),
    );
    expect(soundLoops({ ...STILL, drifting: true, miniTurbo: "orange" })).toEqual(
      new Set(["driftSkid"]),
    );
  });

  it("runs the grass and the sand while the surface holds, with the skid over either", () => {
    expect(soundLoops({ ...STILL, surface: "grass" })).toEqual(new Set(["grass"]));
    expect(soundLoops({ ...STILL, surface: "sand" })).toEqual(new Set(["sand"]));
    expect(soundLoops({ ...STILL, surface: "sand", drifting: true, miniTurbo: "none" })).toEqual(
      new Set(["driftSkid", "sand"]),
    );
  });

  it("runs nothing on asphalt off a drift, nor on a wall or out of the world", () => {
    expect(soundLoops(STILL)).toEqual(new Set());
    expect(soundLoops({ ...STILL, surface: "wall" })).toEqual(new Set());
    expect(soundLoops({ ...STILL, surface: "out of world", airborne: true })).toEqual(new Set());
  });
});

describe("loopPlayback", () => {
  it("pitches the skid by its band's centre at the kart's speed over its centre as rendered, clamped", () => {
    expect(loopPlayback("driftSkid", 15, 30).rate).toBeCloseTo(1, 6);
    expect(loopPlayback("driftSkid", 0, 30).rate).toBeCloseTo(600 / 900, 6);
    expect(loopPlayback("driftSkid", 30, 30).rate).toBeCloseTo(1200 / 900, 6);
    expect(loopPlayback("driftSkid", -30, 30).rate).toBeCloseTo(1200 / 900, 6);
    expect(loopPlayback("driftSkid", 45, 30).rate).toBeCloseTo(1500 / 900, 6);
    expect(loopPlayback("driftSkid", 200, 30).rate).toBe(2);
  });

  it("keeps the grass and the sand at their rendered pitch, whose bands do not move with speed", () => {
    expect(loopPlayback("grass", 0, 30).rate).toBe(1);
    expect(loopPlayback("grass", 45, 30).rate).toBe(1);
    expect(loopPlayback("sand", 0, 30).rate).toBe(1);
    expect(loopPlayback("sand", 45, 30).rate).toBe(1);
  });

  it("swells each loop with speed on its own curve, flat past the boosts' headroom", () => {
    expect(loopPlayback("driftSkid", 0, 30).gain).toBeCloseTo(0.4, 6);
    expect(loopPlayback("driftSkid", 30, 30).gain).toBeCloseTo(1, 6);
    expect(loopPlayback("driftSkid", 45, 30).gain).toBeCloseTo(1.3, 6);
    expect(loopPlayback("driftSkid", 90, 30).gain).toBeCloseTo(1.3, 6);
    expect(loopPlayback("grass", 0, 30).gain).toBeCloseTo(0.3, 6);
    expect(loopPlayback("grass", 30, 30).gain).toBeCloseTo(1, 6);
    expect(loopPlayback("sand", 15, 30).gain).toBeCloseTo(0.65, 6);
  });
});

describe("soundReading", () => {
  const kart: KartTransform = {
    position: [0, 0.3, 0],
    rotation: [0, 0, 0, 1],
    speed: 12,
    surface: "asphalt",
    drift: { direction: "left", chargeSeconds: 1, miniTurbo: "blue" },
    heldItem: null,
    spinOutSeconds: 0.4,
    airborne: true,
    boostSeconds: null,
    start: "clean",
    stallSeconds: null,
  };
  const standing: Standing = {
    position: 3,
    lap: 2,
    checkpoint: 4,
    splineProgress: 100,
    lapTimeSeconds: 10,
    bestLapSeconds: null,
    wrongWay: false,
    finishTimeSeconds: null,
  };
  const racing: Phase = { kind: "racing", elapsedSeconds: 5, graceLeftSeconds: null };

  it("takes the kart's speed, hop, charge and spin from the transform and the lap from the standing", () => {
    expect(soundReading(kart, standing, { item: null, firing: false }, racing)).toEqual({
      speed: 12,
      airborne: true,
      miniTurbo: "blue",
      item: null,
      spunOut: true,
      boostSeconds: null,
      lap: 2,
      finished: false,
      count: "",
      charge: null,
      stalled: false,
      surface: "asphalt",
      drifting: true,
      wrongWay: false,
    });
  });

  it("takes the surface and the wrong-way flag, and whether a drift is under way at any tier", () => {
    const slot = { item: null, firing: false };
    expect(
      soundReading(
        {
          ...kart,
          surface: "grass",
          drift: { direction: "right", chargeSeconds: 0, miniTurbo: "none" },
        },
        { ...standing, wrongWay: true },
        slot,
        racing,
      ),
    ).toMatchObject({ surface: "grass", drifting: true, miniTurbo: "none", wrongWay: true });
    expect(soundReading({ ...kart, drift: null }, standing, slot, racing)).toMatchObject({
      drifting: false,
      miniTurbo: null,
    });
  });

  it("takes the item from the slot, which a room's race fills from the server's events", () => {
    const reading = soundReading(
      { ...kart, drift: null, spinOutSeconds: null, boostSeconds: 0.5 },
      { ...standing, finishTimeSeconds: 61.2 },
      { item: "homing", firing: false },
      racing,
    );
    expect(reading).toMatchObject({
      miniTurbo: null,
      spunOut: false,
      item: "homing",
      boostSeconds: 0.5,
      finished: true,
    });
  });

  it("takes the count the Instrument panel shows and, while it counts, the rocket start's charge", () => {
    const slot = { item: null, firing: false };
    const countingDown: Phase = { kind: "countdown", remainingSeconds: 1.4 };
    expect(
      soundReading({ ...kart, start: "weakBoost" }, standing, slot, countingDown),
    ).toMatchObject({ count: "2", charge: "weakBoost" });
    // From Go the verdict is the boost's or the stall's to tell, not a charge.
    const go: Phase = { kind: "racing", elapsedSeconds: 0.2, graceLeftSeconds: null };
    expect(
      soundReading({ ...kart, start: "stall", stallSeconds: 1.8 }, standing, slot, go),
    ).toMatchObject({ count: "GO", charge: null, stalled: true });
    expect(soundReading(kart, standing, slot, { kind: "finished" })).toMatchObject({
      count: "",
      charge: null,
    });
  });
});

describe("enginePitchHz", () => {
  it("idles at a standstill and climbs with speed either way, past top speed on a boost", () => {
    const idle = enginePitchHz(0, 30);
    const top = enginePitchHz(30, 30);
    expect(idle).toBeGreaterThan(0);
    expect(top).toBeGreaterThan(idle);
    expect(enginePitchHz(15, 30)).toBeCloseTo((idle + top) / 2, 6);
    expect(enginePitchHz(-15, 30)).toBeCloseTo((idle + top) / 2, 6);
    expect(enginePitchHz(39, 30)).toBeGreaterThan(top);
    // Past the headroom left for the boosts, nothing climbs further.
    expect(enginePitchHz(90, 30)).toBe(enginePitchHz(45, 30));
  });

  it("rises through a rocket start's charge at a standstill, with no warning past the full tier", () => {
    const idle = enginePitchHz(0, 30);
    expect(enginePitchHz(0, 30, "clean")).toBe(idle);
    const weak = enginePitchHz(0, 30, "weakBoost");
    const full = enginePitchHz(0, 30, "fullBoost");
    expect(weak).toBeGreaterThan(idle);
    expect(full).toBeGreaterThan(weak);
    expect(full).toBeLessThanOrEqual(enginePitchHz(30, 30));
    expect(enginePitchHz(0, 30, "stall")).toBe(full);
    // Speed still sets the note once the kart is faster than the charge sounds.
    expect(enginePitchHz(30, 30, "weakBoost")).toBe(enginePitchHz(30, 30));
  });
});
