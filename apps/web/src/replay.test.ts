import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { type KartInput, loadBridge } from "./bridge";
import { IDLE_INPUT } from "./input";
import { type ReplayFile, recordReplay } from "./replay";

// The wasm half of the determinism gate (ADR 0002): the committed fixtures
// replay through the wasm build to the hashes they were recorded with. The
// native half is crates/sim/tests/replay.rs against the same files.
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));
const FIXTURES = new URL("../../../assets/replays/", import.meta.url);

function sidecar(track: string): string {
  return readFileSync(
    new URL(`../../../assets/tracks/${track}/${track}.track.json`, import.meta.url),
    "utf8",
  );
}

function fixtures(): { name: string; json: string; track: string; hashes: bigint[] }[] {
  return readdirSync(FIXTURES)
    .filter((name) => name.endsWith(".replay.json"))
    .sort()
    .map((name) => {
      const json = readFileSync(new URL(name, FIXTURES), "utf8");
      const file = JSON.parse(json) as { track: string; hashes: string[] };
      return {
        name,
        json,
        track: file.track,
        hashes: file.hashes.map((hex) => BigInt(`0x${hex}`)),
      };
    });
}

// Every fixture runs in one test, so this gate takes seconds of wasm and more
// when the rest of the suite shares the CI runner. How long it takes is not
// what it checks, and a slow runner must not fail the deploy that depends on it.
const REPLAY_ALL_FIXTURES_TIMEOUT_MS = 60_000;

function hashFromHex(hex: string): bigint {
  return BigInt(`0x${hex}`);
}

describe("replay determinism", () => {
  it(
    "replays every committed fixture in wasm to the hashes it was recorded with",
    async () => {
      const bridge = await loadBridge(wasm);
      const all = fixtures();
      expect(all.length).toBeGreaterThanOrEqual(3);
      for (const fixture of all) {
        expect(fixture.hashes.length, fixture.name).toBeGreaterThan(0);
        expect(bridge.replayHashes(fixture.json, sidecar(fixture.track)), fixture.name).toEqual(
          fixture.hashes,
        );
      }
    },
    REPLAY_ALL_FIXTURES_TIMEOUT_MS,
  );

  it("rejects a replay run on a sidecar that is not its track, with the sim's reason", async () => {
    const bridge = await loadBridge(wasm);
    const [fixture] = fixtures().filter((fixture) => fixture.track === "oval");
    expect(fixture).toBeDefined();
    expect(() => bridge.replayHashes(fixture?.json ?? "", sidecar("bell-deer"))).toThrow(
      /recorded on oval/,
    );
    expect(() => bridge.replayHashes("{}", sidecar("oval"))).toThrow(/replay/);
  });

  it("hashes every 60th tick, as the fixtures were recorded", async () => {
    const bridge = await loadBridge(wasm);
    expect(bridge.replayHashIntervalTicks).toBe(60);
  });
});

describe("replay recorder", () => {
  const THROTTLE: KartInput = { ...IDLE_INPUT, throttle: 1 };
  const TURN: KartInput = { ...THROTTLE, steer: -1 };
  const HOP: KartInput = { ...THROTTLE, hop: true };

  it("records a browser session as a replay the wasm runner reproduces", async () => {
    const bridge = await loadBridge(wasm);
    const oval = sidecar("oval");
    const race = bridge.createRace(9n, 1, 0, oval, "basic");
    const recorder = recordReplay(bridge, 9n, "oval", 0, "basic", race);
    const hashes: bigint[] = [];
    for (let tick = 1; tick <= 150; tick++) {
      const input = tick <= 100 ? THROTTLE : TURN;
      race.step([input], 1 / bridge.ticksPerSecond);
      recorder.record([input]);
      if (tick % bridge.replayHashIntervalTicks === 0) {
        hashes.push(race.stateHash());
      }
    }

    const json = recorder.toJSON();
    const file = JSON.parse(json) as ReplayFile;
    expect(file.version).toBe(1);
    expect(file.seed).toBe("9");
    expect(file.track).toBe("oval");
    expect(file.bots).toBe(0);
    // Consecutive identical inputs are one segment.
    expect(file.segments).toEqual([
      { ticks: 100, inputs: [THROTTLE] },
      { ticks: 50, inputs: [TURN] },
    ]);
    expect(file.hashes).toHaveLength(2);
    expect(file.hashes.map(hashFromHex)).toEqual(hashes);
    expect(bridge.replayHashes(json, oval)).toEqual(hashes);
  });

  it("starts a new segment when any one kart's input changes, buttons included", async () => {
    const bridge = await loadBridge(wasm);
    const race = bridge.createRace(9n, 2, 0, sidecar("oval"), "basic");
    const recorder = recordReplay(bridge, 9n, "oval", 0, "basic", race);
    for (const inputs of [
      [THROTTLE, THROTTLE],
      [THROTTLE, THROTTLE],
      [THROTTLE, TURN],
      [THROTTLE, THROTTLE],
      [HOP, THROTTLE],
    ]) {
      race.step(inputs, 1 / bridge.ticksPerSecond);
      recorder.record(inputs);
    }
    const file = JSON.parse(recorder.toJSON()) as ReplayFile;
    expect(file.segments).toEqual([
      { ticks: 2, inputs: [THROTTLE, THROTTLE] },
      { ticks: 1, inputs: [THROTTLE, TURN] },
      { ticks: 1, inputs: [THROTTLE, THROTTLE] },
      { ticks: 1, inputs: [HOP, THROTTLE] },
    ]);
    expect(() => bridge.replayHashes(recorder.toJSON(), sidecar("oval"))).not.toThrow();
  });

  it("records the bots' count and only the humans' inputs, and the wasm runner reruns the bots", async () => {
    const bridge = await loadBridge(wasm);
    const oval = sidecar("oval");
    const race = bridge.createRace(9n, 1, 2, oval, "basic");
    const recorder = recordReplay(bridge, 9n, "oval", 2, "basic", race);
    for (let tick = 1; tick <= 120; tick++) {
      race.step([THROTTLE], 1 / bridge.ticksPerSecond);
      recorder.record([THROTTLE]);
    }
    const file = JSON.parse(recorder.toJSON()) as ReplayFile;
    expect(file.bots).toBe(2);
    expect(file.segments).toEqual([{ ticks: 120, inputs: [THROTTLE] }]);
    expect(file.hashes).toHaveLength(2);
    expect(bridge.replayHashes(recorder.toJSON(), oval)).toEqual(file.hashes.map(hashFromHex));
    // The hashes are the race's with its bots, not a race of the human alone.
    const alone = bridge.createRace(9n, 1, 0, oval, "basic");
    const aloneRecorder = recordReplay(bridge, 9n, "oval", 0, "basic", alone);
    for (let tick = 1; tick <= 120; tick++) {
      alone.step([THROTTLE], 1 / bridge.ticksPerSecond);
      aloneRecorder.record([THROTTLE]);
    }
    expect((JSON.parse(aloneRecorder.toJSON()) as ReplayFile).hashes).not.toEqual(file.hashes);
  });

  it("keeps a seed a JSON number could not", async () => {
    const bridge = await loadBridge(wasm);
    const seed = 2n ** 64n - 1n;
    const recorder = recordReplay(
      bridge,
      seed,
      "oval",
      0,
      "basic",
      bridge.createRace(seed, 1, 0, sidecar("oval"), "basic"),
    );
    expect((JSON.parse(recorder.toJSON()) as ReplayFile).seed).toBe("18446744073709551615");
  });

  it("starts empty", async () => {
    const bridge = await loadBridge(wasm);
    const recorder = recordReplay(
      bridge,
      1n,
      "oval",
      0,
      "basic",
      bridge.createRace(1n, 1, 0, sidecar("oval"), "basic"),
    );
    expect(JSON.parse(recorder.toJSON())).toMatchObject({ segments: [], hashes: [] });
  });
});
