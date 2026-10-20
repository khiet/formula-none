import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { type KartInput, type KartTransform, loadBridge, type Race } from "./bridge";
import type { Input, KartSnapshot, Snapshot } from "./generated/protocol";
import { IDLE_INPUT } from "./input";

// The real wasm build, loaded from bytes because the default loader fetches
// the module URL and fetch rejects file: URLs in Node. `pnpm build:bridge`
// must have run first. Races run on the oval fixture's sidecar.
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));
const OVAL = readFileSync(
  new URL("../../../assets/tracks/oval/oval.track.json", import.meta.url),
  "utf8",
);
const DT = 1 / 60;
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
    const race = bridge.createRace(7n, 1, 1, OVAL);
    expect(race.phase()).toEqual({ kind: "countdown", remainingSeconds: 3 });
    // A press this early is the stall, and the bot sits the countdown out.
    let ticks = 0;
    while (race.phase().kind === "countdown") {
      race.step([THROTTLE], DT);
      ticks += 1;
    }
    expect(ticks).toBeGreaterThanOrEqual(180);
    expect(ticks).toBeLessThanOrEqual(181);
    expect(race.phase()).toEqual({ kind: "racing", elapsedSeconds: 0, graceLeftSeconds: null });
    expect(race.kartTransforms().map((kart) => kart.speed)).toEqual([0, 0]);
    expect(race.standings().map((kart) => kart.lapTimeSeconds)).toEqual([0, 0]);
    expect(race.standings().map((kart) => kart.finishTimeSeconds)).toEqual([null, null]);

    for (let tick = 0; tick < 30; tick++) {
      race.step([THROTTLE], DT);
    }
    const phase = race.phase();
    expect(phase.kind).toBe("racing");
    if (phase.kind === "racing") {
      expect(phase.elapsedSeconds).toBeCloseTo(0.5, 2);
    }
    const [human, bot] = race.kartTransforms();
    expect(human?.speed).toBe(0);
    expect(bot?.speed).toBeGreaterThan(1);
  });

  it("reports the grace once the bot finishes, its finish time, and the end of the race", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 1, 1, OVAL), 1);
    // The human idles on the grid; the bot laps the oval three times.
    let ticks = 0;
    while (race.standings()[1]?.finishTimeSeconds === null) {
      race.step([IDLE_INPUT], DT);
      ticks += 1;
      expect(ticks, "three laps take under 90 s").toBeLessThan(90 * 60);
    }
    const [human, bot] = race.standings();
    expect(bot?.lap).toBe(bridge.lapsPerRace);
    expect(bot?.finishTimeSeconds).toBeCloseTo(ticks * DT, 1);
    expect(bot?.position).toBe(1);
    expect(human?.finishTimeSeconds).toBeNull();
    const phase = race.phase();
    expect(phase.kind).toBe("racing");
    if (phase.kind === "racing") {
      expect(phase.graceLeftSeconds).toBeCloseTo(20, 4);
    }

    // Twenty seconds of grace, with a tick for the timer's rounding.
    for (let tick = 0; tick < 20 * 60 + 1; tick++) {
      race.step([IDLE_INPUT], DT);
    }
    expect(race.phase()).toEqual({ kind: "finished" });
    expect(race.standings()[0]?.position).toBe(2);
    // Over: nothing moves however the race is stepped.
    const hash = race.stateHash();
    race.step([THROTTLE], DT);
    expect(race.stateHash()).toBe(hash);
  });
});

describe("bridge wrapper", () => {
  it("reports the sim's 60 Hz tick rate and its kart box", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.ticksPerSecond).toBe(60);
    expect(bridge.kartSize.width).toBeCloseTo(1.2);
    expect(bridge.kartSize.height).toBeCloseTo(0.6);
    expect(bridge.kartSize.length).toBeCloseTo(1.8);
  });

  it("steps each kart by its own input and reports transforms in grid order", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 2, 0, OVAL), 2);
    const spawn = race.kartTransforms();
    for (let tick = 0; tick < 60; tick++) {
      race.step([THROTTLE, IDLE_INPUT], DT);
    }
    const after = race.kartTransforms();

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

  it("answers the countdown", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.countdownSeconds).toBe(3);
  });

  it("reports the surface under a kart that steers off the asphalt", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 1, 0, OVAL), 1);
    const seen = new Set<string>();
    for (let tick = 0; tick < 180; tick++) {
      race.step([TURN], DT);
      seen.add(race.kartTransforms()[0]?.surface ?? "missing");
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
    expect(() => bridge.createRace(7n, 1, 0, "{}")).toThrow(/sidecar/);
    expect(() => bridge.createRace(7n, 9, 0, OVAL)).toThrow(/spawn slots/);
  });

  it("replays the same seed and inputs to the same transform", async () => {
    const bridge = await loadBridge(wasm);
    const drive = (): KartTransform[] => {
      const race = started(bridge.createRace(99n, 1, 0, OVAL), 1);
      for (let tick = 0; tick < 120; tick++) {
        race.step([tick < 60 ? THROTTLE : TURN], DT);
      }
      return race.kartTransforms();
    };

    expect(drive()).toEqual(drive());
  });

  it("reports a hop landed into a drift, with its direction and charge tier", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 1, 0, OVAL), 1);
    const hopRight: KartInput = { ...TURN, hop: true };
    for (let tick = 0; tick < 180; tick++) {
      race.step([THROTTLE], DT);
    }
    expect(race.kartTransforms()[0]?.drift).toBeNull();
    // 1.2 s with the button down: past the 0.3 s airtime and the 0.8 s blue
    // charge, short of orange at 1.6 s.
    for (let tick = 0; tick < 72; tick++) {
      race.step([hopRight], DT);
    }
    const drift = race.kartTransforms()[0]?.drift;
    expect(drift?.direction).toBe("right");
    expect(drift?.miniTurbo).toBe("blue");
    expect(drift?.chargeSeconds).toBeGreaterThan(0.8);
    expect(drift?.chargeSeconds).toBeLessThan(1);

    race.step([THROTTLE], DT);
    expect(race.kartTransforms()[0]?.drift).toBeNull();
    expect(race.kartTransforms()[0]?.speed).toBeGreaterThan(30);
  });

  it("refuses an input list that does not match the kart count", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 2, 0, OVAL);
    expect(() => race.step([IDLE_INPUT], DT)).toThrow(RangeError);
    expect(() => race.step([IDLE_INPUT, IDLE_INPUT, IDLE_INPUT], DT)).toThrow(RangeError);
  });

  it("fills the grid behind the human with bots that drive themselves", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.kartsPerRace).toBe(8);
    const race = started(bridge.createRace(7n, 1, bridge.kartsPerRace - 1, OVAL), 1);
    expect(race.kartTransforms()).toHaveLength(8);
    expect(race.standings()).toHaveLength(8);
    // Only the human's input is given; eight would be a caller error.
    expect(() => race.step(Array(8).fill(IDLE_INPUT), DT)).toThrow(RangeError);
    for (let tick = 0; tick < 180; tick++) {
      race.step([IDLE_INPUT], DT);
    }
    const [human, ...bots] = race.kartTransforms();
    expect(human?.speed).toBe(0);
    expect(bots.every((bot) => bot.speed > 20)).toBe(true);
    expect(race.standings()[0]?.position).toBe(8);
    expect(() => bridge.createRace(7n, 1, 8, OVAL)).toThrow(/spawn slots/);
  });
});

describe("standings", () => {
  it("reports a kart on the grid in first place on lap one with no best lap, and the race's lap count", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.lapsPerRace).toBe(3);
    const race = bridge.createRace(7n, 2, 0, OVAL);
    const [pole, second] = race.standings();
    expect(pole).toEqual({
      position: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: expect.any(Number),
      lapTimeSeconds: 0,
      bestLapSeconds: null,
      wrongWay: false,
      finishTimeSeconds: null,
    });
    expect(second?.position).toBe(2);
    // The grid is just behind the start line, at the far end of the lap.
    expect(pole?.splineProgress).toBeGreaterThan(400);
  });

  it("times the lap, crosses gates in order and swaps positions as the sim reports them", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 2, 0, OVAL), 2);
    // Pole idles while the second kart drives up the straight past it and checkpoint 1.
    for (let tick = 0; tick < 180; tick++) {
      race.step([IDLE_INPUT, THROTTLE], DT);
    }
    const [pole, driven] = race.standings();
    expect(driven?.position).toBe(1);
    expect(pole?.position).toBe(2);
    expect(driven?.checkpoint).toBe(1);
    expect(driven?.lapTimeSeconds).toBeCloseTo(3, 1);
    expect(pole?.lapTimeSeconds).toBeCloseTo(3, 1);
    expect(driven?.bestLapSeconds).toBeNull();
    expect(driven?.wrongWay).toBe(false);
  });

  it("raises the wrong-way warning after backing away from the line for the tuned time", async () => {
    const bridge = await loadBridge(wasm);
    const race = started(bridge.createRace(7n, 1, 0, OVAL), 1);
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
    const race = bridge.createRace(7n, 2, 0, OVAL);
    const prediction = bridge.createPrediction(7n, 1, OVAL);
    expect(prediction.kartTransform()).toEqual(race.kartTransforms()[1]);
    expect(prediction.phase()).toEqual({ kind: "countdown", remainingSeconds: 3 });

    // Through the countdown and a second of throttle, as the server would
    // step the same kart from the same inputs.
    for (let sequence = 1; sequence <= 240; sequence++) {
      const input = sequence > 180 ? THROTTLE : IDLE_INPUT;
      prediction.tick(wire(input, sequence));
      race.step([IDLE_INPUT, input], DT);
    }
    expect(prediction.kartTransform()).toEqual(race.kartTransforms()[1]);
    expect(prediction.phase().kind).toBe("racing");
    expect(prediction.correctionsPerSecond()).toBe(0);
  });

  it("is moved by a snapshot that disagrees, and counts the correction", async () => {
    const bridge = await loadBridge(wasm);
    const prediction = bridge.createPrediction(7n, 0, OVAL);
    expect(prediction.interpolatedKartTransforms()).toEqual([]);
    // Idle on the grid through the countdown and a second of the race.
    for (let sequence = 1; sequence <= 240; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    const [x, y, z] = prediction.kartTransform().position;
    const aside: KartSnapshot = {
      position: [x + 1, y, z],
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      heldItem: null,
      spinOutS: null,
    };
    const snapshot: Snapshot = {
      tick: 234,
      acknowledged: [230],
      karts: [aside],
      events: [],
      hazards: [],
    };
    expect(prediction.reconcile(snapshot)).toBeCloseTo(1, 3);
    // The snapshot's karts are drawn too, with the sim's surface under them.
    const [drawn] = prediction.interpolatedKartTransforms();
    expect(drawn?.position[0]).toBeCloseTo(x + 1, 4);
    expect(drawn).toMatchObject({ speed: 0, surface: "asphalt", drift: null });
    expect(prediction.correctionsPerSecond()).toBeCloseTo(1, 3);
    // Blended: drawn where it was, then on to the snapshot's word.
    expect(prediction.kartTransform().position[0]).toBeCloseTo(x, 4);
    for (let sequence = 241; sequence <= 246; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    expect(prediction.kartTransform().position[0]).toBeCloseTo(x + 1, 4);
    expect(() => prediction.reconcile({ ...snapshot, tick: 236, karts: [] })).toThrow(/kart 0/);
  });

  it("carries the item a snapshot says the own kart holds through the prediction", async () => {
    const bridge = await loadBridge(wasm);
    const prediction = bridge.createPrediction(7n, 0, OVAL);
    for (let sequence = 1; sequence <= 10; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    const [x, y, z] = prediction.kartTransform().position;
    const holding: KartSnapshot = {
      position: [x, y, z],
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      heldItem: "homing",
      spinOutS: null,
    };
    prediction.reconcile({
      tick: 10,
      acknowledged: [4],
      karts: [holding],
      events: [],
      hazards: [],
    });
    expect(prediction.kartTransform().heldItem).toBe("homing");
    expect(prediction.interpolatedKartTransforms()[0]?.heldItem).toBe("homing");
  });

  it("takes the spin-out a snapshot says the own kart is in and counts it down", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.spinOutSeconds).toBe(1);
    const prediction = bridge.createPrediction(7n, 0, OVAL);
    // Idle on the grid through the countdown and a second of the race.
    for (let sequence = 1; sequence <= 240; sequence++) {
      prediction.tick(wire(IDLE_INPUT, sequence));
    }
    expect(prediction.kartTransform().spinOutSeconds).toBeNull();
    const [x, y, z] = prediction.kartTransform().position;
    const spun: KartSnapshot = {
      position: [x, y, z],
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      drift: null,
      racePosition: 1,
      lap: 1,
      checkpoint: 0,
      splineProgress: 0,
      wrongWay: false,
      heldItem: null,
      spinOutS: 0.5,
    };
    prediction.reconcile({
      tick: 234,
      acknowledged: [230],
      karts: [spun],
      events: [],
      hazards: [],
    });
    // Input 230 met tick 234, so input 240 meets tick 244: ten ticks of
    // the spin-out have run by the predicted kart's tick.
    expect(prediction.kartTransform().spinOutSeconds).toBeCloseTo(0.5 - 10 * DT, 4);
    expect(prediction.interpolatedKartTransforms()[0]?.spinOutSeconds).toBe(0.5);
  });
});

describe("items", () => {
  it("reports the empty slot, the track's boxes and the tuned respawn", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.itemBoxRespawnSeconds).toBe(3);
    expect(bridge.predictedFireTimeoutSeconds).toBe(1);
    const race = bridge.createRace(7n, 1, 0, OVAL);
    expect(race.kartTransforms()[0]?.heldItem).toBeNull();
    const boxes = race.itemBoxes();
    expect(boxes).toHaveLength(8);
    expect(boxes[0]).toEqual({ position: [-34.5, 0, -70], yaw: expect.closeTo(-Math.PI, 5) });
    expect(race.itemBoxesPresent()).toEqual(Array<boolean>(8).fill(true));
    expect(race.hazards()).toEqual([]);
    expect(bridge.hazardDropDistanceM).toBe(1);
  });
});
