import { describe, expect, it } from "vitest";
import { loopPlayback, type SoundReading, soundCues, soundLoops, soundReading } from "./audio-cues";
import type { KartReadout, Phase, Standing } from "./bridge";
import { kartReadout } from "./kart-readout-fixture";
import { raceReadout } from "./race-readout-fixture";
import type { ItemSlot, RaceCount } from "./race-run";

const STILL: SoundReading = {
  speed: 0,
  airborne: false,
  miniTurbo: null,
  item: null,
  spunOut: false,
  boostSeconds: null,
  lap: 1,
  finished: false,
  count: null,
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

  it("beeps as the race's count changes to 3, 2 and 1, and sounds Go as it shows Go", () => {
    const at = (count: RaceCount): SoundReading => ({ ...STILL, count });
    // The 3 is up from the first reading, so it is heard from nothing.
    expect(soundCues(undefined, at(3))).toEqual(["countdownBeep"]);
    expect(soundCues(at(3), at(3))).toEqual([]);
    expect(soundCues(at(3), at(2))).toEqual(["countdownBeep"]);
    expect(soundCues(at(2), at(1))).toEqual(["countdownBeep"]);
    expect(soundCues(at(1), at("go"))).toEqual(["go"]);
    expect(soundCues(at("go"), at("go"))).toEqual([]);
    expect(soundCues(at("go"), at(null))).toEqual([]);
    expect(soundCues(at(null), at(null))).toEqual([]);
  });

  it("plays the hop as the kart leaves the ground, not while it is up or as it lands", () => {
    const up = { ...STILL, airborne: true };
    expect(soundCues(STILL, up)).toEqual(["hop"]);
    expect(soundCues(up, up)).toEqual([]);
    expect(soundCues(up, STILL)).toEqual([]);
  });

  it("plays the mini-turbo when a charged drift ends into its boost, and nothing for an uncharged one", () => {
    const boosting = { ...STILL, boostSeconds: 0.6 };
    expect(soundCues({ ...STILL, miniTurbo: "orange" }, boosting)).toEqual(["miniTurbo"]);
    expect(soundCues({ ...STILL, miniTurbo: "red" }, boosting)).toEqual(["miniTurbo"]);
    expect(soundCues({ ...STILL, miniTurbo: "none" }, STILL)).toEqual([]);
    // A boost from a pad with no drift ending is not a release either.
    expect(soundCues(STILL, boosting)).toEqual(["boostPad"]);
    // Let go while a pad's boost still runs: the boost lengthens, so it is heard.
    expect(
      soundCues(
        { ...STILL, miniTurbo: "orange", boostSeconds: 0.2 },
        { ...STILL, boostSeconds: 0.6 },
      ),
    ).toEqual(["miniTurbo"]);
    // Charging through the tiers is the chime's tell, not the release's.
    expect(soundCues({ ...STILL, miniTurbo: "none" }, { ...STILL, miniTurbo: "orange" })).toEqual([
      "tierChime",
    ]);
  });

  it("plays the tier chime as the drift charges to orange and again to red, the same chime both times", () => {
    const at = (miniTurbo: SoundReading["miniTurbo"]): SoundReading => ({
      ...STILL,
      miniTurbo,
      drifting: miniTurbo !== null,
    });
    expect(soundCues(STILL, at("none"))).toEqual([]);
    expect(soundCues(at("none"), at("none"))).toEqual([]);
    expect(soundCues(at("none"), at("orange"))).toEqual(["tierChime"]);
    expect(soundCues(at("orange"), at("orange"))).toEqual([]);
    expect(soundCues(at("orange"), at("red"))).toEqual(["tierChime"]);
    expect(soundCues(at("red"), at("red"))).toEqual([]);
    // The drift ending, charged or not, is no chime.
    expect(soundCues(at("red"), STILL)).toEqual([]);
    expect(soundCues(at("orange"), { ...STILL, boostSeconds: 0.6 })).toEqual(["miniTurbo"]);
  });

  it("plays the boost pad as a boost starts or lengthens that no release and no item fired explains", () => {
    const boosting = { ...STILL, boostSeconds: 0.6 };
    expect(soundCues(STILL, boosting)).toEqual(["boostPad"]);
    expect(soundCues(boosting, boosting)).toEqual([]);
    // A boost ticking down is the same boost; a second pad lengthens it.
    expect(soundCues(boosting, { ...STILL, boostSeconds: 0.5 })).toEqual([]);
    expect(soundCues({ ...STILL, boostSeconds: 0.2 }, boosting)).toEqual(["boostPad"]);
    // A charged drift's release is the mini-turbo, not a pad.
    expect(soundCues({ ...STILL, miniTurbo: "red", drifting: true }, boosting)).toEqual([
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
      soundCues({ ...STILL, count: 1, charge: "fullBoost" }, { ...boosting, count: "go" }),
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
    expect(soundCues({ ...STILL, miniTurbo: "red" }, { ...STILL, spunOut: true })).toEqual(["hit"]);
    // Slowed below drift speed, or respawned: the drift just ends.
    expect(soundCues({ ...STILL, miniTurbo: "orange" }, STILL)).toEqual([]);
  });

  it("plays the mini-turbo as a rocket start's charge becomes its boost at GO", () => {
    const charging = (charge: SoundReading["charge"]): SoundReading => ({
      ...STILL,
      count: 1,
      charge,
    });
    const go: SoundReading = { ...STILL, count: "go" };
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
    const stalled: SoundReading = { ...STILL, count: "go", stalled: true };
    expect(soundCues({ ...STILL, count: 1, charge: "stall" }, stalled)).toEqual(["go", "sputter"]);
    expect(soundCues(stalled, stalled)).toEqual([]);
    expect(soundCues(stalled, { ...STILL, count: "go" })).toEqual([]);
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
    expect(soundLoops({ ...STILL, drifting: true, miniTurbo: "red" })).toEqual(
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
    expect(soundLoops({ ...STILL, surface: "out_of_world", airborne: true })).toEqual(new Set());
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

  it("swells each loop with speed on its own curve from the file's level at 15 m/s, flat past the boosts' headroom", () => {
    expect(loopPlayback("driftSkid", 15, 30).gain).toBeCloseTo(1, 6);
    expect(loopPlayback("driftSkid", 0, 30).gain).toBeCloseTo(0.4 / 0.7, 6);
    expect(loopPlayback("driftSkid", 30, 30).gain).toBeCloseTo(1 / 0.7, 6);
    expect(loopPlayback("driftSkid", 45, 30).gain).toBeCloseTo(1.3 / 0.7, 6);
    expect(loopPlayback("driftSkid", 90, 30).gain).toBeCloseTo(1.3 / 0.7, 6);
    expect(loopPlayback("grass", 15, 30).gain).toBeCloseTo(1, 6);
    expect(loopPlayback("grass", 0, 30).gain).toBeCloseTo(0.3 / 0.65, 6);
    expect(loopPlayback("grass", 30, 30).gain).toBeCloseTo(1 / 0.65, 6);
    expect(loopPlayback("sand", 15, 30).gain).toBeCloseTo(1, 6);
  });
});

describe("soundReading", () => {
  const kart: KartReadout = kartReadout({
    position: [0, 0.3, 0],
    speed: 12,
    drift: { direction: "left", chargeS: 1 },
    miniTurbo: "orange",
    spinOutS: 0.4,
    airborne: true,
  });
  const standing: Standing = {
    position: 3,
    lap: 2,
    checkpoint: 4,
    splineProgress: 100,
    lapTimeS: 10,
    bestLapS: null,
    wrongWay: false,
    finishTimeS: null,
  };
  const racing: Phase = { kind: "racing", elapsedSeconds: 5, capLeftSeconds: null };
  /** The reading of the one kart of a race at `kart` on `standing` holding `itemSlot`. */
  const read = (kart: KartReadout, standing: Standing, itemSlot: ItemSlot, phase: Phase) =>
    soundReading(raceReadout({ karts: [kart], standings: [standing], itemSlot, phase }), 0);

  it("takes the kart's speed, hop, charge and spin from the transform and the lap from the standing", () => {
    expect(read(kart, standing, { item: null, firing: false }, racing)).toEqual({
      speed: 12,
      airborne: true,
      miniTurbo: "orange",
      item: null,
      spunOut: true,
      boostSeconds: null,
      lap: 2,
      finished: false,
      count: null,
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
      read(
        {
          ...kart,
          surface: "grass",
          drift: { direction: "right", chargeS: 0 },
          miniTurbo: "none",
        },
        { ...standing, wrongWay: true },
        slot,
        racing,
      ),
    ).toMatchObject({ surface: "grass", drifting: true, miniTurbo: "none", wrongWay: true });
    expect(read({ ...kart, drift: null }, standing, slot, racing)).toMatchObject({
      drifting: false,
      miniTurbo: null,
    });
  });

  it("takes the item from the slot, which a room's race fills from the server's events", () => {
    const reading = read(
      { ...kart, drift: null, spinOutS: null, boostS: 0.5 },
      { ...standing, finishTimeS: 61.2 },
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

  it("takes the race's count and, while it counts, the rocket start's charge", () => {
    const slot = { item: null, firing: false };
    const countingDown: Phase = { kind: "countdown", remainingSeconds: 1.4 };
    expect(read({ ...kart, start: "weakBoost" }, standing, slot, countingDown)).toMatchObject({
      count: 2,
      charge: "weakBoost",
    });
    // From Go the verdict is the boost's or the stall's to tell, not a charge.
    const go: Phase = { kind: "racing", elapsedSeconds: 0.2, capLeftSeconds: null };
    expect(read({ ...kart, start: "stall", stallS: 1.8 }, standing, slot, go)).toMatchObject({
      count: "go",
      charge: null,
      stalled: true,
    });
    expect(read(kart, standing, slot, { kind: "finished" })).toMatchObject({
      count: null,
      charge: null,
    });
  });
});
