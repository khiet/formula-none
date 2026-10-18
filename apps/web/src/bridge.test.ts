import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { type KartInput, type KartTransform, loadBridge } from "./bridge";
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
    const race = bridge.createRace(7n, 2, OVAL);
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

  it("reports the surface under a kart that steers off the asphalt", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 1, OVAL);
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
    expect(bridge.encodePing(42)).toEqual(new Uint8Array([0, 42]));
    expect(bridge.decodePong(new Uint8Array([0, 42]))).toBe(42);
    expect(() => bridge.decodePong(new Uint8Array([0xff, 0xff]))).toThrow(/malformed/);
    expect(bridge.webTransportTimeoutMs).toBe(3_000);
    expect(bridge.pingTiming).toEqual({ timeoutMs: 5_000, resendMs: 500 });
  });

  it("rejects a sidecar the sim cannot race on with the sim's reason", async () => {
    const bridge = await loadBridge(wasm);
    expect(() => bridge.createRace(7n, 1, "{}")).toThrow(/sidecar/);
    expect(() => bridge.createRace(7n, 9, OVAL)).toThrow(/spawn slots/);
  });

  it("replays the same seed and inputs to the same transform", async () => {
    const bridge = await loadBridge(wasm);
    const drive = (): KartTransform[] => {
      const race = bridge.createRace(99n, 1, OVAL);
      for (let tick = 0; tick < 120; tick++) {
        race.step([tick < 60 ? THROTTLE : TURN], DT);
      }
      return race.kartTransforms();
    };

    expect(drive()).toEqual(drive());
  });

  it("reports a hop landed into a drift, with its direction and charge tier", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 1, OVAL);
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
    const race = bridge.createRace(7n, 2, OVAL);
    expect(() => race.step([IDLE_INPUT], DT)).toThrow(RangeError);
    expect(() => race.step([IDLE_INPUT, IDLE_INPUT, IDLE_INPUT], DT)).toThrow(RangeError);
  });
});
