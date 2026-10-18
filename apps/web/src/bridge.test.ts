import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { type KartInput, loadBridge } from "./bridge";

// The real wasm build, loaded from bytes because Node has no fetch for
// import.meta.url. `pnpm build:bridge` must have run first.
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));
const DT = 1 / 60;
const IDLE: KartInput = { steer: 0, throttle: 0, brake: 0 };
const THROTTLE: KartInput = { steer: 0, throttle: 1, brake: 0 };

describe("bridge wrapper", () => {
  it("reports the sim's 60 Hz tick rate", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.ticksPerSecond).toBe(60);
  });

  it("steps each kart by its own input and reports transforms in grid order", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(7n, 2);
    const spawn = race.kartTransforms();
    for (let tick = 0; tick < 60; tick++) {
      race.step([THROTTLE, IDLE], DT);
    }
    const after = race.kartTransforms();

    expect(after).toHaveLength(2);
    expect(spawn[0]?.position[0]).toBeLessThan(spawn[1]?.position[0] ?? Number.NaN);
    expect(after[0]?.position[2]).toBeLessThan((spawn[0]?.position[2] ?? 0) - 1);
    expect(after[1]?.position[0]).toBeCloseTo(spawn[1]?.position[0] ?? Number.NaN, 2);
    expect(after[1]?.position[2]).toBeCloseTo(spawn[1]?.position[2] ?? Number.NaN, 2);
    expect(after[0]?.rotation).toEqual([0, 0, 0, 1]);
  });

  it("replays the same seed and inputs to the same transform", async () => {
    const bridge = await loadBridge(wasm);
    const drive = (): ReturnType<typeof race.kartTransforms> => {
      const race = bridge.createRace(99n, 1);
      for (let tick = 0; tick < 120; tick++) {
        race.step([{ steer: tick < 60 ? 0 : 1, throttle: 1, brake: 0 }], DT);
      }
      return race.kartTransforms();
    };
    const race = bridge.createRace(99n, 1);
    void race;

    expect(drive()).toEqual(drive());
  });
});
