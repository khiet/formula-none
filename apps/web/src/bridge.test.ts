import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { type KartInput, type KartReadout, loadBridge, type Race, type Wheel } from "./bridge";
import type { Input, KartSnapshot, Snapshot } from "./generated/protocol";
import { IDLE_INPUT } from "./input";
import { START_LIGHTS } from "./instrument-panel";
import { landedItem } from "./race-run";

// The real wasm build, loaded from bytes because the default loader fetches
// the module URL and fetch rejects file: URLs in Node. `pnpm build:bridge`
// must have run first. Races run on the oval fixture's sidecar, and on
// Bell Deer's for the runaway wheel.
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));
const OVAL = readFileSync(
  new URL("../../../assets/tracks/oval/oval.track.json", import.meta.url),
  "utf8",
);
const BELL_DEER = readFileSync(
  new URL("../../../assets/tracks/bell-deer/bell-deer.track.json", import.meta.url),
  "utf8",
);
const DT = 1 / 60;
/** The countdown's ticks: five start lights, one a second. */
const COUNTDOWN_TICKS = 5 * 60;
const THROTTLE: KartInput = { ...IDLE_INPUT, throttle: 1 };
const TURN: KartInput = { ...THROTTLE, steer: 1 };

/** Step `race` idle through its countdown, so the karts can be driven. */
function started(race: Race, humanCount: number): Race {
  const idle = Array<KartInput>(humanCount).fill(IDLE_INPUT);
  while (race.phase().kind === "countdown") {
    race.step(idle, DT);
  }
  return race;
}

describe("race phase", () => {
  it("starts at the top of the countdown, holds the karts until Go, then races", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 1, 1, OVAL, "basic", "off");
    expect(race.phase()).toEqual({ kind: "countdown", remainingSeconds: 5 });
    // A press this early is the stall, and the bot sits the countdown out.
    let ticks = 0;
    while (race.phase().kind === "countdown") {
      race.step([THROTTLE], DT);
      ticks += 1;
    }
    expect(ticks).toBeGreaterThanOrEqual(COUNTDOWN_TICKS);
    expect(ticks).toBeLessThanOrEqual(COUNTDOWN_TICKS + 1);
    expect(race.phase()).toEqual({ kind: "racing", elapsedSeconds: 0, capLeftSeconds: null });
    expect(race.kartReadouts().map((kart) => kart.speed)).toEqual([0, 0]);
    expect(race.standings().map((kart) => kart.lapTimeS)).toEqual([0, 0]);
    expect(race.standings().map((kart) => kart.finishTimeS)).toEqual([null, null]);

    for (let tick = 0; tick < 30; tick++) {
      race.step([THROTTLE], DT);
    }
    const phase = race.phase();
    expect(phase.kind).toBe("racing");
    if (phase.kind === "racing") {
      expect(phase.elapsedSeconds).toBeCloseTo(0.5, 2);
    }
    const [human, bot] = race.kartReadouts();
    expect(human?.speed).toBe(0);
    expect(bot?.speed).toBeGreaterThan(1);
  });

  it("reports the cap from the first finish, the outro once all have finished, and the end of the race", async () => {
    const bridge = await loadBridge(wasm);
    // With no humans, the bots' own finishes start the cap and end the race.
    const race = started(bridge.createRace(7n, 0, 2, OVAL, "basic", "off"), 0);
    const finishers = () => race.standings().filter((standing) => standing.finishTimeS !== null);
    let ticks = 0;
    while (finishers().length === 0) {
      race.step([], DT);
      ticks += 1;
      expect(ticks, "the race's laps take under 90 s").toBeLessThan(90 * 60);
    }
    const [first] = finishers();
    expect(first?.lap).toBe(bridge.lapsPerRace);
    expect(first?.finishTimeS).toBeCloseTo(ticks * DT, 1);
    expect(first?.position).toBe(1);
    const phase = race.phase();
    expect(phase.kind).toBe("racing");
    if (phase.kind === "racing") {
      expect(phase.capLeftSeconds).toBeCloseTo(60, 4);
    }

    while (finishers().length < 2) {
      race.step([], DT);
    }
    expect(race.phase()).toEqual({ kind: "outro" });

    // Three seconds of outro, with a tick for the timer's rounding.
    for (let tick = 0; tick < 3 * 60 + 1; tick++) {
      race.step([], DT);
    }
    expect(race.phase()).toEqual({ kind: "finished" });
    // Over: nothing moves however the race is stepped.
    const hash = race.stateHash();
    race.step([], DT);
    expect(race.stateHash()).toBe(hash);
  });
});

describe("bridge wrapper", () => {
  it("reports the sim's 60 Hz tick rate, its kart box and the runaway wheel's reach", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.ticksPerSecond).toBe(60);
    expect(bridge.kartSize.width).toBeCloseTo(1.2);
    expect(bridge.kartSize.height).toBeCloseTo(0.6);
    expect(bridge.kartSize.length).toBeCloseTo(1.8);
    expect(bridge.runawayWheelRadiusMetres).toBeCloseTo(1.2);
  });

  it("steps each kart by its own input and reports transforms in grid order", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 2, 0, OVAL, "basic", "off"), 2);
    const spawn = race.kartReadouts();
    for (let tick = 0; tick < 60; tick++) {
      race.step([THROTTLE, IDLE_INPUT], DT);
    }
    const after = race.kartReadouts();

    expect(after).toHaveLength(2);
    // The oval's grid: slot 0 is the left column, both face -Z on asphalt.
    expect(spawn[0]?.position[0]).toBeLessThan(spawn[1]?.position[0] ?? Number.NaN);
    expect(spawn.map((kart) => kart.surface)).toEqual(["asphalt", "asphalt"]);
    expect(after[0]?.position[2]).toBeLessThan((spawn[0]?.position[2] ?? 0) - 1);
    expect(after[1]?.position[0]).toBeCloseTo(spawn[1]?.position[0] ?? Number.NaN, 2);
    expect(after[1]?.position[2]).toBeCloseTo(spawn[1]?.position[2] ?? Number.NaN, 2);
    expect(after[0]?.rotation).toEqual([0, 0, 0, 1]);
    expect(after[0]?.speed).toBeGreaterThan(5);
    expect(after[1]?.speed).toBe(0);
  });

  it("answers the countdown: one second per start light the panel draws", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.countdownSeconds).toBe(START_LIGHTS);
  });

  it("reads the chase camera's tuning from the sim: top speed, pull-back, shake and boost kick", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.cameraTuning).toEqual({
      topSpeedMetresPerSecond: 30,
      pullBackMetres: 1,
      shakeSeconds: expect.closeTo(0.3, 5),
      shakeAmplitudeMetres: expect.any(Number),
      fieldOfViewDegrees: 60,
      boostKick: expect.closeTo(0.12, 5),
      boostKickEaseSeconds: expect.closeTo(0.3, 5),
    });
    expect(bridge.cameraTuning.shakeAmplitudeMetres).toBeGreaterThan(0);
  });

  it("reports the surface under a kart that steers off the asphalt", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 1, 0, OVAL, "basic", "off"), 1);
    const seen = new Set<string>();
    for (let tick = 0; tick < 180; tick++) {
      race.step([TURN], DT);
      seen.add(race.kartReadouts()[0]?.surface ?? "missing");
    }
    expect(seen.has("asphalt")).toBe(true);
    expect(seen.has("sand")).toBe(true);
    expect(seen.has("missing")).toBe(false);
  });

  it("encodes a Ping and decodes a Pong as the protocol does, and reports the transport timeout", async () => {
    const bridge = await loadBridge(wasm);
    // Postcard: variant 0, then the nonce as a varint (crates/protocol/tests).
    expect(bridge.encodeClientMessage({ ping: { nonce: 42 } })).toEqual(new Uint8Array([0, 42]));
    expect(bridge.decodeServerMessage(new Uint8Array([0, 42]))).toEqual({ pong: { nonce: 42 } });
    expect(() => bridge.decodeServerMessage(new Uint8Array([0xff, 0xff]))).toThrow(/malformed/);
    expect(bridge.webTransportTimeoutMs).toBe(3_000);
    expect(bridge.pingTiming).toEqual({ timeoutMs: 5_000, resendMs: 500 });
    expect(bridge.reconnectTiming).toEqual({ silenceMs: 2_000, retryMs: 1_000 });
  });

  it("reports the room's tuned bounds for the Title Scene to validate against", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.nicknameChars).toEqual({ min: 2, max: 12 });
    expect(bridge.roomCodeLength).toBe(4);
  });

  it("rejects a sidecar the sim cannot race on with the sim's reason", async () => {
    const bridge = await loadBridge(wasm);
    expect(() => bridge.createRace(7n, 1, 0, "{}", "basic", "off")).toThrow(/sidecar/);
    expect(() => bridge.createRace(7n, 9, 0, OVAL, "basic", "off")).toThrow(/spawn slots/);
  });

  it("replays the same seed and inputs to the same transform", async () => {
    const bridge = await loadBridge(wasm);
    const drive = (): KartReadout[] => {
      const race = started(bridge.createRace(99n, 1, 0, OVAL, "basic", "off"), 1);
      for (let tick = 0; tick < 120; tick++) {
        race.step([tick < 60 ? THROTTLE : TURN], DT);
      }
      return race.kartReadouts();
    };

    expect(drive()).toEqual(drive());
  });

  it("reports a hop landed into a drift, with its direction and charge tier", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 1, 0, OVAL, "basic", "off"), 1);
    const hopRight: KartInput = { ...TURN, hop: true };
    for (let tick = 0; tick < 180; tick++) {
      race.step([THROTTLE], DT);
    }
    expect(race.kartReadouts()[0]?.drift).toBeNull();
    expect(race.kartReadouts()[0]?.airborne).toBe(false);
    // The hop's tell: in the air from the press through the 0.3 s airtime.
    race.step([hopRight], DT);
    expect(race.kartReadouts()[0]?.airborne).toBe(true);
    // 0.75 s with the button down: past the 0.3 s airtime and the 0.3 s orange
    // charge, short of red at 0.6 s.
    for (let tick = 0; tick < 44; tick++) {
      race.step([hopRight], DT);
    }
    expect(race.kartReadouts()[0]?.airborne).toBe(false);
    const kart = race.kartReadouts()[0];
    const drift = kart?.drift;
    expect(drift?.direction).toBe("right");
    expect(kart?.miniTurbo).toBe("orange");
    expect(drift?.chargeS).toBeGreaterThan(0.3);
    expect(drift?.chargeS).toBeLessThan(0.6);

    race.step([THROTTLE], DT);
    expect(race.kartReadouts()[0]?.drift).toBeNull();
    expect(race.kartReadouts()[0]?.speed).toBeGreaterThan(30);
    // The release's tell: the orange boost's 0.6 s, less the tick just stepped,
    // whatever the oval's boost pad had left.
    expect(race.kartReadouts()[0]?.boostS).toBeCloseTo(0.6 - DT, 3);
  });

  it("reports a rocket start's charge through the countdown, and its stall or boost from Go", async () => {
    const bridge = await loadBridge(wasm);
    // The throttle held from the top of the countdown: the tiers come as
    // the hold lengthens, and the hold is too long by Go.
    const stalling = bridge.createRace(7n, 1, 0, OVAL, "basic", "off");
    const tiers: string[] = [];
    while (stalling.phase().kind === "countdown") {
      stalling.step([THROTTLE], DT);
      const { start } = stalling.kartReadouts()[0] ?? { start: "clean" };
      if (tiers.at(-1) !== start) tiers.push(start);
    }
    expect(tiers).toEqual(["clean", "weakBoost", "fullBoost", "stall"]);
    stalling.step([THROTTLE], DT);
    expect(stalling.kartReadouts()[0]?.stallS).toBeCloseTo(2 - DT, 4);
    expect(stalling.kartReadouts()[0]?.speed).toBe(0);

    // Held 1.75 s, from the first half of the fourth light: the full boost off the line.
    const boosting = bridge.createRace(7n, 1, 0, OVAL, "basic", "off");
    for (let tick = 0; tick < COUNTDOWN_TICKS - 105; tick++) boosting.step([IDLE_INPUT], DT);
    while (boosting.phase().kind === "countdown") boosting.step([THROTTLE], DT);
    expect(boosting.kartReadouts()[0]?.start).toBe("fullBoost");
    boosting.step([THROTTLE], DT);
    expect(boosting.kartReadouts()[0]?.stallS).toBeNull();
    expect(boosting.kartReadouts()[0]?.boostS).toBeCloseTo(1.5 - DT, 4);
  });

  it("refuses an input list that does not match the kart count", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 2, 0, OVAL, "basic", "off");
    expect(() => race.step([IDLE_INPUT], DT)).toThrow(RangeError);
    expect(() => race.step([IDLE_INPUT, IDLE_INPUT, IDLE_INPUT], DT)).toThrow(RangeError);
  });

  it("fills the grid behind the human with bots that drive themselves", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.kartsPerRace).toBe(8);
    // A seed that rolls no bot the start boost, so none shoves the idle human.
    const race = started(
      bridge.createRace(181n, 1, bridge.kartsPerRace - 1, OVAL, "basic", "off"),
      1,
    );
    expect(race.kartReadouts()).toHaveLength(8);
    expect(race.standings()).toHaveLength(8);
    // Only the human's input is given; eight would be a caller error.
    expect(() => race.step(Array(8).fill(IDLE_INPUT), DT)).toThrow(RangeError);
    for (let tick = 0; tick < 180; tick++) {
      race.step([IDLE_INPUT], DT);
    }
    const [human, ...bots] = race.kartReadouts();
    expect(human?.speed).toBe(0);
    expect(bots.every((bot) => bot.speed > 20)).toBe(true);
    expect(race.standings()[0]?.position).toBe(8);
    expect(() => bridge.createRace(7n, 1, 8, OVAL, "basic", "off")).toThrow(/spawn slots/);
  });
});

describe("standings", () => {
  it("reports a kart on the grid in first place on lap one with no best lap, and the race's lap count", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.lapsPerRace).toBe(3);
    const race = bridge.createRace(7n, 2, 0, OVAL, "basic", "off");
    const [pole, second] = race.standings();
    expect(pole).toEqual({
      position: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: expect.any(Number),
      lapTimeS: 0,
      bestLapS: null,
      wrongWay: false,
      finishTimeS: null,
      stops: 0,
      bestStopS: null,
    });
    expect(second?.position).toBe(2);
    // The grid is just behind the start line, at the far end of the lap.
    expect(pole?.splineProgress).toBeGreaterThan(400);
  });

  it("times the lap, crosses gates in order and swaps positions as the sim reports them", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 2, 0, OVAL, "basic", "off"), 2);
    // Pole idles while the second kart drives up the straight past it and checkpoint 1.
    for (let tick = 0; tick < 180; tick++) {
      race.step([IDLE_INPUT, THROTTLE], DT);
    }
    const [pole, driven] = race.standings();
    expect(driven?.position).toBe(1);
    expect(pole?.position).toBe(2);
    expect(driven?.checkpoint).toBe(1);
    expect(driven?.lapTimeS).toBeCloseTo(3, 1);
    expect(pole?.lapTimeS).toBeCloseTo(3, 1);
    expect(driven?.bestLapS).toBeNull();
    expect(driven?.wrongWay).toBe(false);
  });

  it("raises the wrong-way warning after backing away from the line for the tuned time", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 1, 0, OVAL, "basic", "off"), 1);
    const brake = { ...IDLE_INPUT, brake: 1 };
    // Two seconds of reversing, as the sim's tuning has it, plus a tick.
    for (let tick = 0; tick < 121; tick++) {
      race.step([brake], DT);
    }
    expect(race.standings()[0]?.wrongWay).toBe(true);
    // Rolling forward again clears it.
    for (let tick = 0; tick < 60; tick++) {
      race.step([THROTTLE], DT);
    }
    expect(race.standings()[0]?.wrongWay).toBe(false);
  });
});

describe("the pit lane", () => {
  it("reads across the seam: a kart past the entry line is in it, one on the grid is not", async () => {
    const bridge = await loadBridge(wasm);
    const sidecar = JSON.parse(BELL_DEER);
    const [position] = sidecar.pit_lane.path.slice(60);
    sidecar.spawn_grid[0] = { position, yaw: sidecar.pit_lane.entry.yaw };
    const race = started(bridge.createRace(7n, 2, 0, JSON.stringify(sidecar), "basic", "off"), 2);
    expect(race.kartReadouts().map((kart) => kart.inPitLane)).toEqual([true, false]);
  });
});

describe("tyre life", () => {
  it("falls from 100 as a kart drives with Tyres On, and holds there with Off", async () => {
    const bridge = await loadBridge(wasm);
    const on = started(bridge.createRace(7n, 1, 0, OVAL, "basic", "on"), 1);
    const off = started(bridge.createRace(7n, 1, 0, OVAL, "basic", "off"), 1);
    expect(on.kartReadouts()[0]?.tyreLife).toBe(100);
    for (let tick = 0; tick < 120; tick++) {
      on.step([THROTTLE], DT);
      off.step([THROTTLE], DT);
    }
    expect(on.kartReadouts()[0]?.tyreLife).toBeLessThan(100);
    expect(off.kartReadouts()[0]?.tyreLife).toBe(100);
  });

  it("refuses an unknown Tyres pick", async () => {
    const bridge = await loadBridge(wasm);
    // @ts-expect-error: a pick the types rule out, as a stale caller could send.
    expect(() => bridge.createRace(7n, 1, 0, OVAL, "basic", "slick")).toThrow(/slick/);
  });

  it("predicts the own kart's wear as the server wears it", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 1, 0, OVAL, "basic", "on");
    const prediction = bridge.createPrediction(7n, 0, OVAL, "basic", "on");
    for (let sequence = 1; sequence <= COUNTDOWN_TICKS + 60; sequence++) {
      const input = sequence > COUNTDOWN_TICKS ? THROTTLE : IDLE_INPUT;
      prediction.tick({
        sequence,
        steer: 0,
        throttle: input === THROTTLE ? 255 : 0,
        brake: 0,
        hop: false,
        useItem: false,
        holdItem: false,
      });
      race.step([input], DT);
    }
    const server = race.kartReadouts()[0]?.tyreLife;
    expect(server).toBeLessThan(100);
    expect(prediction.kartReadout().tyreLife).toBe(server);
  });
});

describe("prediction", () => {
  /** The wire input for `input` under `sequence`; the throttle and steer used here quantise exactly. */
  const wire = (input: KartInput, sequence: number): Input => ({
    sequence,
    steer: Math.round(input.steer * 127),
    throttle: Math.round(input.throttle * 255),
    brake: Math.round(input.brake * 255),
    hop: input.hop,
    useItem: input.useItem,
    holdItem: input.holdItem,
  });

  it("stands the kart on its grid slot and drives it by the sim's rules from the inputs sent", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 2, 0, OVAL, "basic", "off");
    const prediction = bridge.createPrediction(7n, 1, OVAL, "basic", "off");
    // The prediction races its kart alone, so its race position counts
    // only that kart.
    const alone = (kart: KartReadout | undefined) => ({ ...kart, racePosition: 1 });
    expect(prediction.kartReadout()).toEqual(alone(race.kartReadouts()[1]));
    expect(prediction.phase()).toEqual({ kind: "countdown", remainingSeconds: 5 });

    // Through the countdown and a second of throttle, as the server would
    // step the same kart from the same inputs.
    for (let sequence = 1; sequence <= COUNTDOWN_TICKS + 60; sequence++) {
      const input = sequence > COUNTDOWN_TICKS ? THROTTLE : IDLE_INPUT;
      prediction.tick(wire(input, sequence));
      race.step([IDLE_INPUT, input], DT);
    }
    expect(prediction.kartReadout()).toEqual(alone(race.kartReadouts()[1]));
    expect(prediction.phase().kind).toBe("racing");
    expect(prediction.correctionsPerSecond()).toBe(0);
  });

  it("is moved by a snapshot that disagrees, and counts the correction", async () => {
    const bridge = await loadBridge(wasm);
    const prediction = bridge.createPrediction(7n, 0, OVAL, "basic", "off");
    expect(prediction.interpolatedKartReadouts()).toEqual([]);
    // Idle on the grid through the countdown and a second of the race, so
    // one correction is one a second. The tick that goes is the first raced.
    let last = 0;
    while (prediction.phase().kind === "countdown") prediction.tick(wire(IDLE_INPUT, ++last));
    for (let raced = 1; raced < 60; raced++) prediction.tick(wire(IDLE_INPUT, ++last));
    const [x, y, z] = prediction.kartReadout().position;
    const aside: KartSnapshot = {
      position: [x + 1, y, z],
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      bump: null,
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      itemSlot: "empty",
      coins: 0,
      tyreLife: 100,
      spinOutS: null,
      start: "clean",
      stallS: null,
      boost: null,
      starS: null,
      slipstream: null,
      stop: null,
    };
    const snapshot: Snapshot = {
      tick: last - 6,
      acknowledged: [last - 10],
      karts: [aside],
      events: [],
      hazards: [],
      projectiles: [],
      coinsPresent: [],
    };
    expect(prediction.reconcile(snapshot)).toBeCloseTo(1, 3);
    // The snapshot's karts are drawn too, with the sim's surface under them.
    const [drawn] = prediction.interpolatedKartReadouts();
    expect(drawn?.position[0]).toBeCloseTo(x + 1, 4);
    expect(drawn).toMatchObject({ speed: 0, surface: "asphalt", drift: null });
    expect(prediction.correctionsPerSecond()).toBeCloseTo(1, 3);
    // Blended: drawn where it was, then on to the snapshot's word.
    expect(prediction.kartReadout().position[0]).toBeCloseTo(x, 4);
    for (let sequence = last + 1; sequence <= last + 6; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    expect(prediction.kartReadout().position[0]).toBeCloseTo(x + 1, 4);
    expect(() => prediction.reconcile({ ...snapshot, tick: last - 4, karts: [] })).toThrow(
      /kart 0/,
    );
  });

  it("carries the item a snapshot says the own kart holds through the prediction", async () => {
    const bridge = await loadBridge(wasm);
    const prediction = bridge.createPrediction(7n, 0, OVAL, "basic", "off");
    for (let sequence = 1; sequence <= 10; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    const [x, y, z] = prediction.kartReadout().position;
    const holding: KartSnapshot = {
      position: [x, y, z],
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      bump: null,
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      itemSlot: { held: "homing" },
      coins: 0,
      tyreLife: 100,
      spinOutS: null,
      start: "clean",
      stallS: null,
      boost: null,
      starS: null,
      slipstream: null,
      stop: null,
    };
    prediction.reconcile({
      tick: 10,
      acknowledged: [4],
      karts: [holding],
      events: [],
      hazards: [],
      projectiles: [],
      coinsPresent: [],
    });
    expect(prediction.kartReadout().itemSlot).toEqual({ held: "homing" });
    expect(prediction.interpolatedKartReadouts()[0]?.itemSlot).toEqual({ held: "homing" });
  });

  it("takes the spin-out a snapshot says the own kart is in and counts it down", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.spinOutSeconds).toBe(1);
    const prediction = bridge.createPrediction(7n, 0, OVAL, "basic", "off");
    // Idle on the grid through the countdown and a second of the race.
    for (let sequence = 1; sequence <= COUNTDOWN_TICKS + 60; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    expect(prediction.kartReadout().spinOutS).toBeNull();
    const [x, y, z] = prediction.kartReadout().position;
    const spun: KartSnapshot = {
      position: [x, y, z],
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      bump: null,
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      itemSlot: "empty",
      coins: 0,
      tyreLife: 100,
      spinOutS: 0.5,
      start: "clean",
      stallS: null,
      boost: null,
      starS: null,
      slipstream: null,
      stop: null,
    };
    prediction.reconcile({
      tick: COUNTDOWN_TICKS + 54,
      acknowledged: [COUNTDOWN_TICKS + 50],
      karts: [spun],
      events: [],
      hazards: [],
      projectiles: [],
      coinsPresent: [],
    });
    // Input Go+50 met tick Go+54, so input Go+60 meets tick Go+64: ten ticks of
    // the spin-out have run by the predicted kart's tick.
    expect(prediction.kartReadout().spinOutS).toBeCloseTo(0.5 - 10 * DT, 4);
    expect(prediction.interpolatedKartReadouts()[0]?.spinOutS).toBe(0.5);
  });

  it("draws another kart's boost, and the tier it burns in, from the snapshots", async () => {
    const bridge = await loadBridge(wasm);
    const prediction = bridge.createPrediction(7n, 0, OVAL, "basic", "off");
    for (let sequence = 1; sequence <= COUNTDOWN_TICKS + 60; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    const kart = (
      position: [number, number, number],
      boost: KartSnapshot["boost"],
    ): KartSnapshot => ({
      position,
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      bump: null,
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      itemSlot: "empty",
      coins: 0,
      tyreLife: 100,
      spinOutS: null,
      start: "clean",
      stallS: null,
      boost,
      starS: null,
      slipstream: null,
      stop: null,
    });
    const [x, y, z] = prediction.kartReadout().position;
    prediction.reconcile({
      tick: COUNTDOWN_TICKS + 54,
      acknowledged: [COUNTDOWN_TICKS + 50, 0],
      karts: [kart([x, y, z], null), kart([32, 0.3, 6], { leftS: 0.5, tier: "orange" })],
      events: [],
      hazards: [],
      projectiles: [],
      coinsPresent: [],
    });
    expect(prediction.interpolatedKartReadouts()[1]).toMatchObject({
      boostS: 0.5,
      boostTier: "orange",
    });
  });

  it("takes the rocket start verdict and the stall a snapshot says, and draws another kart's from the snapshots", async () => {
    const bridge = await loadBridge(wasm);
    const prediction = bridge.createPrediction(7n, 0, OVAL, "basic", "off");
    // Idle on the grid through the countdown and a second of the race: a
    // clean start predicted, which the server decided a stall.
    for (let sequence = 1; sequence <= COUNTDOWN_TICKS + 60; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    expect(prediction.kartReadout()).toMatchObject({ start: "clean", stallS: null });
    const [x, y, z] = prediction.kartReadout().position;
    const kart = (position: [number, number, number], stallS: number): KartSnapshot => ({
      position,
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      bump: null,
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      itemSlot: "empty",
      coins: 0,
      tyreLife: 100,
      spinOutS: null,
      start: "stall",
      stallS,
      boost: null,
      starS: null,
      slipstream: null,
      stop: null,
    });
    prediction.reconcile({
      tick: COUNTDOWN_TICKS + 54,
      acknowledged: [COUNTDOWN_TICKS + 50, 0],
      karts: [kart([x, y, z], 1.0), kart([32, 0.3, 6], 0.5)],
      events: [],
      hazards: [],
      projectiles: [],
      coinsPresent: [],
    });
    // Input Go+50 met tick Go+54, so input Go+60 meets tick Go+64: ten ticks of
    // the stall have run by the predicted kart's tick.
    expect(prediction.kartReadout().start).toBe("stall");
    expect(prediction.kartReadout().stallS).toBeCloseTo(1.0 - 10 * DT, 4);
    expect(prediction.interpolatedKartReadouts()[1]).toMatchObject({
      start: "stall",
      stallS: 0.5,
    });
  });
});

describe("items", () => {
  it("reports the empty slot, the track's boxes and the tuned respawn", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.itemBoxRespawnSeconds).toBe(3);
    expect(bridge.predictedFireTimeoutSeconds).toBe(1);
    const race = bridge.createRace(7n, 1, 0, OVAL, "basic", "off");
    expect(race.kartReadouts()[0]?.itemSlot).toBe("empty");
    const boxes = race.itemBoxes();
    expect(boxes).toHaveLength(8);
    expect(boxes[0]).toEqual({ position: [-34.5, 0, -70], yaw: expect.closeTo(-Math.PI, 5) });
    expect(race.itemBoxesPresent()).toEqual(Array<boolean>(8).fill(true));
    expect(race.hazards()).toEqual([]);
    expect(race.projectiles()).toEqual([]);
    expect(bridge.hazardDropDistanceMetres).toBe(1);
  });

  it("reports each box a kart takes on the step it takes it, the one that rolls and the one that gives nothing", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.itemRollSeconds).toBe(2.5);
    const sidecar = JSON.parse(OVAL) as {
      spawn_grid: { position: [number, number, number] }[];
      item_boxes: { position: [number, number, number]; yaw: number }[];
    };
    // Two boxes under the grid slot: a kart takes one a step, so the first
    // is taken at Go and rolls, and the second the step after, into a full slot.
    const under = sidecar.spawn_grid[0]?.position ?? [0, 0, 0];
    sidecar.item_boxes.push({ position: under, yaw: 0 }, { position: under, yaw: 0 });
    const race = started(bridge.createRace(7n, 1, 0, JSON.stringify(sidecar), "basic", "off"), 1);
    expect(race.boxesTakenLastStep(0)).toBe(0);

    race.step([IDLE_INPUT], DT);
    expect(race.boxesTakenLastStep(0)).toBe(1);
    const rolled = race.kartReadouts()[0]?.itemSlot;
    expect(rolled).toEqual({
      rolling: { item: expect.any(String), rollLeftS: expect.closeTo(2.5, 5) },
    });
    race.step([IDLE_INPUT], DT);
    expect(race.boxesTakenLastStep(0)).toBe(1);
    expect(race.itemBoxesPresent().slice(-2)).toEqual([false, false]);
    race.step([IDLE_INPUT], DT);
    expect(race.boxesTakenLastStep(0)).toBe(0);
  });

  it("reports the track's coins, a kart taking the one under it at Go and carrying it", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.coinCap).toBe(10);
    expect(bridge.coinRespawnSeconds).toBe(3);
    expect(bridge.coinTopSpeeds).toHaveLength(11);
    expect(bridge.coinTopSpeeds[0]).toBeCloseTo(30);
    expect(bridge.coinTopSpeeds[10]).toBeCloseTo(33);
    const sidecar = JSON.parse(OVAL) as {
      spawn_grid: { position: [number, number, number] }[];
      coins: { position: [number, number, number]; yaw: number }[];
    };
    const own = sidecar.coins.length;
    const under = sidecar.spawn_grid[0]?.position ?? [0, 0, 0];
    sidecar.coins.push({ position: under, yaw: 0 });
    const race = started(bridge.createRace(7n, 1, 0, JSON.stringify(sidecar), "basic", "off"), 1);
    const coins = race.coins();
    expect(coins).toHaveLength(own + 1);
    expect(coins[own]).toEqual({ position: under, yaw: 0 });
    expect(race.coinsPresent()).toEqual(Array<boolean>(own + 1).fill(true));
    expect(race.kartReadouts()[0]?.coins).toBe(0);

    race.step([IDLE_INPUT], DT);
    expect(race.kartReadouts()[0]?.coins).toBe(1);
    expect(race.coinsPresent()[own]).toBe(false);
    expect(race.coinsPresent().filter(Boolean)).toHaveLength(own);
  });

  it("reports a projectile in flight from the sim, and from the snapshots as the interpolation draws it", async () => {
    const bridge = await loadBridge(wasm);
    // Two karts race solo through the bridge: a projectile fired by kart 1
    // at kart 0 is read back with its owner and target. The seeds are
    // tried until a kart on a box under its slot rolls one; the sim's own
    // tests cover its flight.
    const sidecar = JSON.parse(OVAL) as {
      spawn_grid: { position: [number, number, number] }[];
      item_boxes: { position: [number, number, number]; yaw: number }[];
    };
    sidecar.item_boxes.push({ position: sidecar.spawn_grid[1]?.position ?? [0, 0, 0], yaw: 0 });
    const boxed = JSON.stringify(sidecar);
    const tick = 1 / bridge.ticksPerSecond;
    const fire: KartInput = { ...IDLE_INPUT, useItem: true };
    let fired = false;
    for (let seed = 0n; seed < 50n && !fired; seed++) {
      const race = bridge.createRace(seed, 2, 0, boxed, "basic", "off");
      while (race.phase().kind === "countdown") race.step([IDLE_INPUT, IDLE_INPUT], tick);
      race.step([IDLE_INPUT, IDLE_INPUT], tick);
      const rolled = race.kartReadouts()[1]?.itemSlot;
      if (
        typeof rolled !== "object" ||
        !("rolling" in rolled) ||
        rolled.rolling.item !== "homing"
      ) {
        continue;
      }
      // Kart 0 drives off while the projectile rolls, and it is fired once landed.
      while (landedItem(race.kartReadouts()[1]?.itemSlot ?? "empty") === null) {
        race.step([THROTTLE, IDLE_INPUT], tick);
      }
      race.step([THROTTLE, fire], tick);
      const [projectile] = race.projectiles();
      expect(projectile).toMatchObject({ owner: 1, target: 0 });
      expect(projectile?.position[0]).toBeCloseTo(32, 0);
      fired = true;
    }
    expect(fired, "some seed rolls kart 1 a projectile").toBe(true);

    const prediction = bridge.createPrediction(7n, 0, OVAL, "basic", "off");
    expect(prediction.interpolatedProjectiles()).toEqual([]);
    const snapshot = (tick: number, x: number): Snapshot => ({
      tick,
      acknowledged: [0],
      karts: [
        {
          position: [28, 0.3, 4],
          rotation: [0, 0, 0, 1],
          velocity: [0, 0, 0],
          bump: null,
          drift: null,
          racePosition: 1,
          lap: 1,
          checkpoint: 0,
          splineProgress: 0,
          wrongWay: false,
          itemSlot: "empty",
          coins: 0,
          tyreLife: 100,
          spinOutS: null,
          start: "clean",
          stallS: null,
          boost: null,
          starS: null,
          slipstream: null,
          stop: null,
        },
      ],
      events: [],
      hazards: [],
      projectiles: [{ position: [x, 0, -10], owner: 1, target: 0 }],
      coinsPresent: [],
    });
    for (let t = 0; t < 6; t++) prediction.reconcile(snapshot(2 * t, t));
    const idle: Input = {
      sequence: 0,
      steer: 0,
      throttle: 0,
      brake: 0,
      hop: false,
      useItem: false,
      holdItem: false,
    };
    for (let t = 0; t < 12; t++) prediction.tick({ ...idle, sequence: t + 1 });
    const [drawn] = prediction.interpolatedProjectiles();
    expect(drawn).toMatchObject({ owner: 1, target: 0 });
    // Drawn the tuned delay behind the newest, between two snapshots.
    expect(drawn?.position[0]).toBeGreaterThan(0);
    expect(drawn?.position[0]).toBeLessThan(5);
  });
});

describe("runaway wheel", () => {
  it("reports the wheel quiet through the countdown, the flag up before the first pass and the wheel rolling through it, as the prediction does", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 1, 0, BELL_DEER, "basic", "off");
    const prediction = bridge.createPrediction(7n, 0, BELL_DEER, "basic", "off");
    // Stepped until the first pass is half way across, read off the wheel
    // itself so a retuned cycle needs no change here.
    const halfWay = () => {
      const [wheel] = race.wheels();
      return wheel?.kind === "rolling" && wheel.fraction >= 0.5;
    };
    const seen = new Set<string>();
    let sequence = 0;
    while (!halfWay()) {
      race.step([IDLE_INPUT], DT);
      sequence += 1;
      prediction.tick({
        sequence,
        steer: 0,
        throttle: 0,
        brake: 0,
        hop: false,
        useItem: false,
        holdItem: false,
      });
      const [wheel] = race.wheels();
      if (race.phase().kind === "countdown") expect(wheel).toEqual({ kind: "quiet" });
      seen.add(wheel?.kind ?? "none");
      expect(prediction.wheels()).toEqual(race.wheels());
    }
    expect([...seen]).toEqual(["quiet", "warning", "rolling"]);
    const [rolling] = race.wheels();
    if (rolling?.kind !== "rolling") throw new Error("not rolling");
    expect(rolling.legFraction).toBeGreaterThanOrEqual(0);
    expect(rolling.legFraction).toBeLessThanOrEqual(1);
    // The crossing it rolls over, as the sidecar has it: the walls run along its yaw.
    expect(rolling.crossingYaw).toBeCloseTo(JSON.parse(BELL_DEER).wheel_crossings[0].yaw, 5);
    // The race's seed lands the bounces: a race from another, stepped as
    // far, has its wheel somewhere else on the approach.
    const other = bridge.createRace(8n, 1, 0, BELL_DEER, "basic", "off");
    for (let step = 0; step < sequence; step += 1) other.step([IDLE_INPUT], DT);
    const [elsewhere] = other.wheels();
    if (elsewhere?.kind !== "rolling") throw new Error("not rolling");
    const apart = Math.hypot(
      elsewhere.position[0] - rolling.position[0],
      elsewhere.position[2] - rolling.position[2],
    );
    expect(apart).toBeGreaterThan(0.5);
  });

  it("through the warning puts the wheel at the wall it then rolls in from, facing the way it rolls", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 1, 0, BELL_DEER, "basic", "off");
    const [crossing] = JSON.parse(BELL_DEER).wheel_crossings;
    let waiting: Extract<Wheel, { kind: "warning" }> | undefined;
    let [wheel] = race.wheels();
    while (wheel?.kind !== "rolling") {
      if (wheel?.kind === "warning") waiting = wheel;
      race.step([IDLE_INPUT], DT);
      [wheel] = race.wheels();
    }
    if (waiting === undefined) throw new Error("no warning");
    // At the wall: the crossing's half width from its centre, on its line.
    const across = Math.hypot(
      waiting.position[0] - crossing.position[0],
      waiting.position[2] - crossing.position[2],
    );
    expect(across).toBeCloseTo(crossing.half_width, 1);
    // A tick's roll from where it waited, the same way.
    const rolled = Math.hypot(
      wheel.position[0] - waiting.position[0],
      wheel.position[2] - waiting.position[2],
    );
    expect(rolled).toBeLessThan(0.5);
    expect(waiting.yaw).toBeCloseTo(wheel.yaw, 4);
  });
});
