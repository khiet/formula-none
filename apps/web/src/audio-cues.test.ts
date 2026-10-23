import { describe, expect, it } from "vitest";
import { enginePitchHz, type SoundReading, soundCues, soundReading } from "./audio-cues";
import type { KartTransform, Standing } from "./bridge";

const STILL: SoundReading = {
  speed: 0,
  airborne: false,
  miniTurbo: null,
  item: null,
  spunOut: false,
  boostSeconds: null,
  lap: 1,
  finished: false,
};

describe("soundCues", () => {
  it("is silent with no change, and before there is a reading to change from", () => {
    expect(soundCues(STILL, STILL)).toEqual([]);
    expect(soundCues(undefined, { ...STILL, airborne: true, item: "boost" })).toEqual([]);
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
    expect(soundCues(STILL, boosting)).toEqual([]);
    // Let go while a pad's boost still runs: the boost lengthens, so it is heard.
    expect(
      soundCues(
        { ...STILL, miniTurbo: "blue", boostSeconds: 0.2 },
        { ...STILL, boostSeconds: 0.6 },
      ),
    ).toEqual(["miniTurbo"]);
    // Charging through the tiers is the sparks' tell, not a sound.
    expect(soundCues({ ...STILL, miniTurbo: "none" }, { ...STILL, miniTurbo: "blue" })).toEqual([]);
    expect(soundCues({ ...STILL, miniTurbo: "blue" }, { ...STILL, miniTurbo: "orange" })).toEqual(
      [],
    );
  });

  it("plays no mini-turbo for a charged drift that ends without its boost", () => {
    // Spun out mid-charge: the hit, and only the hit.
    expect(soundCues({ ...STILL, miniTurbo: "orange" }, { ...STILL, spunOut: true })).toEqual([
      "hit",
    ]);
    // Slowed below drift speed, or respawned: the drift just ends.
    expect(soundCues({ ...STILL, miniTurbo: "blue" }, STILL)).toEqual([]);
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

  it("plays the lap as the lap count rises and as the kart finishes", () => {
    expect(soundCues(STILL, { ...STILL, lap: 2 })).toEqual(["lap"]);
    expect(soundCues({ ...STILL, lap: 3 }, { ...STILL, lap: 3, finished: true })).toEqual(["lap"]);
    expect(
      soundCues({ ...STILL, lap: 3, finished: true }, { ...STILL, lap: 3, finished: true }),
    ).toEqual([]);
  });

  it("reports every change of one frame together", () => {
    const after = { ...STILL, airborne: true, item: "boost" as const, lap: 2 };
    expect(soundCues(STILL, after)).toEqual(["hop", "itemPickup", "lap"]);
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

  it("takes the kart's speed, hop, charge and spin from the transform and the lap from the standing", () => {
    expect(soundReading(kart, standing, { item: null, firing: false })).toEqual({
      speed: 12,
      airborne: true,
      miniTurbo: "blue",
      item: null,
      spunOut: true,
      boostSeconds: null,
      lap: 2,
      finished: false,
    });
  });

  it("takes the item from the slot, which a room's race fills from the server's events", () => {
    const reading = soundReading(
      { ...kart, drift: null, spinOutSeconds: null, boostSeconds: 0.5 },
      { ...standing, finishTimeSeconds: 61.2 },
      { item: "homing", firing: false },
    );
    expect(reading).toMatchObject({
      miniTurbo: null,
      spunOut: false,
      item: "homing",
      boostSeconds: 0.5,
      finished: true,
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
});
