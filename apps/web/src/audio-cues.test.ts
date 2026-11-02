import { describe, expect, it } from "vitest";
import {
  distanceLevel,
  droneBeep,
  hornBlasts,
  loopPlayback,
  SOUND_AWAY_LOWPASS_HZ,
  type SoundReading,
  soundCues,
  soundLoops,
  soundReading,
  starMusic,
} from "./audio-cues";
import type { KartReadout, Phase, Standing } from "./bridge";
import { reelFrame } from "./item-reel";
import { kartReadout } from "./kart-readout-fixture";
import { raceReadout } from "./race-readout-fixture";
import { EMPTY_SLOT, type ItemSlot, type RaceCount } from "./race-run";

const STILL: SoundReading = {
  speed: 0,
  airborne: false,
  miniTurbo: null,
  item: null,
  coins: 0,
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
  boxesTaken: 0,
  rolling: false,
  reelStep: null,
  starSeconds: null,
  nearestStar: null,
  blasts: [],
  incomingDroneMetres: null,
};

describe("soundCues", () => {
  it("is silent with no change, and before there is a reading to change from", () => {
    expect(soundCues(STILL, STILL)).toEqual([]);
    expect(soundCues(undefined, { ...STILL, airborne: true, boxesTaken: 1 })).toEqual([]);
  });

  it("beeps as each of the five start lights comes on, and sounds Go as they go out", () => {
    const at = (count: RaceCount): SoundReading => ({ ...STILL, count });
    // The first light is on from the first reading, so it is heard from nothing.
    expect(soundCues(undefined, at(5))).toEqual(["countdownBeep"]);
    expect(soundCues(at(5), at(5))).toEqual([]);
    const counts: RaceCount[] = [5, 4, 3, 2, 1, "go"];
    const heard = counts
      .slice(1)
      .flatMap((count, index) => soundCues(at(counts[index] ?? null), at(count)));
    expect(heard).toEqual([...Array(4).fill("countdownBeep"), "go"]);
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

  it("plays the pickup for every box taken, the one into a full slot that rolls nothing too", () => {
    const one = { ...STILL, boxesTaken: 1 };
    expect(soundCues(STILL, one)).toEqual(["itemPickup"]);
    expect(soundCues(one, one)).toEqual([]);
    const held = { ...one, item: "boost" as const };
    expect(soundCues(held, { ...held, boxesTaken: 2 })).toEqual(["itemPickup"]);
  });

  it("clicks as the reel steps on, once a frame however far it went, and not as it starts or settles", () => {
    const at = (reelStep: number | null): SoundReading => ({
      ...STILL,
      rolling: reelStep !== null,
      reelStep,
    });
    expect(soundCues({ ...STILL, boxesTaken: 0 }, { ...at(0), boxesTaken: 1 })).toEqual([
      "itemPickup",
    ]);
    expect(soundCues(at(0), at(0))).toEqual([]);
    expect(soundCues(at(0), at(1))).toEqual(["itemRollTick"]);
    expect(soundCues(at(3), at(5))).toEqual(["itemRollTick"]);
    expect(soundCues(at(20), at(20))).toEqual([]);
  });

  it("plays the landing as the rolled item lands, nothing as the slot fills, and the fire as it empties", () => {
    const rolling: SoundReading = { ...STILL, rolling: true, reelStep: 20 };
    const held = { ...STILL, item: "hazard" as const };
    expect(soundCues(rolling, held)).toEqual(["itemRolled"]);
    expect(soundCues(STILL, held)).toEqual([]);
    expect(soundCues(held, held)).toEqual([]);
    expect(soundCues(held, STILL)).toEqual(["itemFire"]);
  });

  it("plays the coin pickup as the coins carried rise", () => {
    const one = { ...STILL, coins: 1 };
    expect(soundCues(STILL, one)).toEqual(["coinPickup"]);
    expect(soundCues(one, one)).toEqual([]);
  });

  it("plays the coin loss after the hit as a spin-out costs coins, and no loss with none to lose", () => {
    const four = { ...STILL, coins: 4 };
    expect(soundCues(four, { ...STILL, coins: 1, spunOut: true })).toEqual(["hit", "coinLoss"]);
    expect(soundCues(STILL, { ...STILL, spunOut: true })).toEqual(["hit"]);
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

  it("plays the star's own fire as a star leaves the slot, in place of the item fire", () => {
    const held = { ...STILL, item: "star" as const };
    const run = { ...STILL, starSeconds: 5.98, boostSeconds: 0.48 };
    expect(soundCues(held, run)).toEqual(["starFire"]);
    // In a room's race the prediction starts the run before the server
    // confirms the fire: the fire sounds once, with the run, the kick is no
    // pad, and the slot emptying on the confirmation adds nothing.
    expect(soundCues(held, { ...run, item: "star" })).toEqual(["starFire"]);
    expect(soundCues({ ...run, item: "star" }, { ...run, starSeconds: 5.5 })).toEqual([]);
    expect(soundCues({ ...STILL, item: "boost" }, { ...STILL, boostSeconds: 1.5 })).toEqual([
      "itemFire",
    ]);
  });

  it("plays the fire again for a second star, which puts the seconds back up, and not for a correction", () => {
    const run = (starSeconds: number): SoundReading => ({ ...STILL, starSeconds });
    // A run ticking down is the same run.
    expect(soundCues(run(4), run(3.98))).toEqual([]);
    // A correction can raise it a little; that restarts nothing.
    expect(soundCues(run(3), run(3.4))).toEqual([]);
    // A second star puts it back to the start.
    expect(soundCues(run(3), run(3.6))).toEqual(["starFire"]);
    expect(soundCues({ ...run(2.9), item: "star" }, run(5.98))).toEqual(["starFire"]);
    // A run appearing from nothing is a fire too; a rejoin's first reading
    // has nothing to change from, so it is heard without one.
    expect(soundCues(STILL, run(4))).toEqual(["starFire"]);
    expect(soundCues(undefined, run(4))).toEqual([]);
  });

  it("plays the star's end as the run runs out, and nothing but the finish when the finish ends it", () => {
    const run = { ...STILL, starSeconds: 0.02 };
    expect(soundCues(run, STILL)).toEqual(["starEnd"]);
    expect(soundCues(run, { ...STILL, finished: true })).toEqual(["finish"]);
    expect(soundCues(STILL, STILL)).toEqual([]);
  });

  it("plays the air horn in place of the item fire as a horn leaves the slot, once", () => {
    const held: SoundReading = { ...STILL, item: "horn" };
    expect(soundCues(held, STILL)).toEqual(["airHorn"]);
    expect(soundCues(STILL, STILL)).toEqual([]);
    expect(soundCues({ ...STILL, item: "hazard" }, STILL)).toEqual(["itemFire"]);
  });

  it("reports every change of one frame together", () => {
    const after = { ...STILL, airborne: true, boxesTaken: 1, lap: 2 };
    expect(soundCues(STILL, after)).toEqual(["hop", "itemPickup", "lap"]);
  });
});

describe("hornBlasts", () => {
  const heard = (id: number, kart: number, metres: number) => ({ id, kart, metres });

  it("hears each other kart's blast once, as it first appears, however long it lasts", () => {
    const first = heard(1, 2, 8);
    const second = heard(2, 5, 30);
    expect(hornBlasts(undefined, { ...STILL, blasts: [first] })).toEqual([first]);
    expect(hornBlasts({ ...STILL, blasts: [first] }, { ...STILL, blasts: [first] })).toEqual([]);
    expect(
      hornBlasts({ ...STILL, blasts: [first] }, { ...STILL, blasts: [first, second] }),
    ).toEqual([second]);
    expect(hornBlasts({ ...STILL, blasts: [first, second] }, STILL)).toEqual([]);
  });
});

describe("droneBeep", () => {
  it("starts at 40 m, every 0.35 s at the warning's edge, closing to every 0.1 s at the kart as the pitch rises by half", () => {
    expect(droneBeep(null)).toBeNull();
    expect(droneBeep(40.5)).toBeNull();
    expect(droneBeep(40)).toEqual({ gapS: 0.35, rate: 1 });
    const halfway = droneBeep(20);
    expect(halfway?.gapS).toBeCloseTo(0.225, 6);
    expect(halfway?.rate).toBeCloseTo(1.25, 6);
    expect(droneBeep(0)).toEqual({ gapS: 0.1, rate: 1.5 });
  });
});

describe("starMusic", () => {
  const RUN_S = 6;

  it("plays the own run's music from the top at a fire, joined further in after a rejoin", () => {
    expect(starMusic({ ...STILL, starSeconds: 6 }, RUN_S)).toEqual({
      kart: null,
      secondsIn: 0,
      level: 1,
      lowpassHz: null,
    });
    expect(starMusic({ ...STILL, starSeconds: 2.5 }, RUN_S)?.secondsIn).toBeCloseTo(3.5, 6);
    expect(starMusic(STILL, RUN_S)).toBeNull();
  });

  it("plays only the nearest other star kart's music, at its point in the run, by distance and lowpassed", () => {
    const nearest = { kart: 4, seconds: 4, metres: 35 };
    expect(starMusic({ ...STILL, nearestStar: nearest }, RUN_S)).toEqual({
      kart: 4,
      secondsIn: 2,
      level: distanceLevel(35),
      lowpassHz: SOUND_AWAY_LOWPASS_HZ,
    });
    // The own run outranks every other kart's.
    expect(starMusic({ ...STILL, starSeconds: 1, nearestStar: nearest }, RUN_S)?.kart).toBeNull();
  });

  it("is full within 10 m, a straight line to silent at 60 m", () => {
    expect(distanceLevel(0)).toBe(1);
    expect(distanceLevel(10)).toBe(1);
    expect(distanceLevel(35)).toBeCloseTo(0.5, 6);
    expect(distanceLevel(45)).toBeCloseTo(0.3, 6);
    expect(distanceLevel(60)).toBe(0);
    expect(distanceLevel(100)).toBe(0);
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
    coins: 3,
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
    expect(read(kart, standing, EMPTY_SLOT, racing)).toEqual({
      speed: 12,
      airborne: true,
      miniTurbo: "orange",
      item: null,
      coins: 3,
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
      boxesTaken: 0,
      rolling: false,
      reelStep: null,
      starSeconds: null,
      nearestStar: null,
      blasts: [],
      incomingDroneMetres: null,
    });
  });

  it("takes the own run's seconds, and the nearest other star kart by distance", () => {
    const own = kartReadout({ position: [0, 0.3, 0], starS: 2 });
    const far = kartReadout({ position: [40, 0.3, 0], starS: 5 });
    const near = kartReadout({ position: [0, 0.3, -12], starS: 1.5 });
    const off = kartReadout({ position: [1, 0.3, -1] });
    const race = raceReadout({
      karts: [own, far, near, off],
      standings: [standing, standing, standing, standing],
    });
    expect(soundReading(race, 0)).toMatchObject({
      starSeconds: 2,
      nearestStar: { kart: 2, seconds: 1.5, metres: 12 },
    });
    expect(soundReading(race, 3)).toMatchObject({
      starSeconds: null,
      nearestStar: { kart: 0, seconds: 2, metres: Math.hypot(1, 1) },
    });
    expect(soundReading(raceReadout({ karts: [off, off] }), 0).nearestStar).toBeNull();
  });

  it("takes the other karts' blasts by distance, never the own kart's, and the nearest drone chasing the own kart", () => {
    const own = kartReadout({ position: [0, 0.3, 0] });
    const other = kartReadout({ position: [30, 0.3, 0] });
    const race = raceReadout({
      karts: [own, other],
      standings: [standing, standing],
      blasts: [
        { id: 1, kart: 0, position: [0, 0.3, 0], elapsedS: 0.1 },
        { id: 2, kart: 1, position: [30, 0.3, -40], elapsedS: 0 },
      ],
      projectiles: [
        { position: [0, 0, -45], owner: 1, target: 0 },
        { position: [0, 0, -20], owner: 1, target: 0 },
        { position: [0, 0, -5], owner: 0, target: 1 },
      ],
    });
    expect(soundReading(race, 0)).toMatchObject({
      blasts: [{ id: 2, kart: 1, metres: 50 }],
      incomingDroneMetres: 20,
    });
    expect(soundReading(race, 1)).toMatchObject({
      blasts: [{ id: 1, kart: 0, metres: 30 }],
      incomingDroneMetres: Math.hypot(30, 5),
    });
    expect(soundReading(raceReadout({ karts: [own] }), 0)).toMatchObject({
      blasts: [],
      incomingDroneMetres: null,
    });
  });

  it("takes the boxes taken, and the roll and the reel's step from the slot", () => {
    const rolling = { item: "boost" as const, elapsedS: 0, rollS: 2.5 };
    expect(read(kart, standing, { ...EMPTY_SLOT, rolling, boxesTaken: 2 }, racing)).toMatchObject({
      boxesTaken: 2,
      rolling: true,
      reelStep: 0,
      item: null,
    });
    const later = read(
      kart,
      standing,
      { ...EMPTY_SLOT, rolling: { ...rolling, elapsedS: 1 } },
      racing,
    );
    expect(later.reelStep).toBe(reelFrame(1, 2.5).step);
    expect(later.reelStep).toBeGreaterThan(0);
  });

  it("takes the surface and the wrong-way flag, and whether a drift is under way at any tier", () => {
    const slot = EMPTY_SLOT;
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
      { ...EMPTY_SLOT, item: "homing" },
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
    const slot = EMPTY_SLOT;
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
