import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { loadBridge } from "./bridge";
import { IDLE_INPUT } from "./input";
import { localRaceRun } from "./local-race";
import { EMPTY_SLOT, ITEM_LANDING_FLASH_S, type RaceRun } from "./race-run";

// The real wasm build, as bridge.test.ts loads it; `pnpm build:bridge`
// must have run first.
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));
const OVAL = readFileSync(
  new URL("../../../assets/tracks/oval/oval.track.json", import.meta.url),
  "utf8",
);
const DT = 1 / 60;

/** The oval with `count` more item boxes under kart 0's grid slot. */
function boxesUnderTheGrid(count: number): string {
  const sidecar = JSON.parse(OVAL) as {
    spawn_grid: { position: [number, number, number] }[];
    item_boxes: { position: [number, number, number]; yaw: number }[];
  };
  const under = sidecar.spawn_grid[0]?.position ?? [0, 0, 0];
  for (let box = 0; box < count; box++) sidecar.item_boxes.push({ position: under, yaw: 0 });
  return JSON.stringify(sidecar);
}

/** Tick `run` idle through its countdown; the next tick is the first a kart can take a box on. */
function started(run: RaceRun): void {
  while (run.readout().phase.kind === "countdown") run.tick(IDLE_INPUT, DT);
}

describe("localRaceRun's item slot", () => {
  it("rolls a box's item for the tuned roll, lands it with the flash, and counts the box", async () => {
    const bridge = await loadBridge(wasm);
    const run = localRaceRun(bridge, "oval", boxesUnderTheGrid(1));
    started(run);
    expect(run.readout().itemSlot).toEqual(EMPTY_SLOT);

    run.tick(IDLE_INPUT, DT);
    const taken = run.readout().itemSlot;
    expect(taken).toEqual({
      ...EMPTY_SLOT,
      rolling: { item: expect.any(String), elapsedS: 0, rollS: 2.5 },
      boxesTaken: 1,
    });
    const rolled = taken.rolling?.item;

    // The sim sums the roll in f32, so it may land a tick past 2.5 s.
    let ticks = 0;
    while (run.readout().itemSlot.rolling !== null && ticks <= 2.5 * 60) {
      ticks++;
      run.tick(IDLE_INPUT, DT);
      const { rolling } = run.readout().itemSlot;
      if (rolling !== null) expect(rolling.elapsedS).toBeCloseTo(ticks * DT, 4);
    }
    expect(ticks).toBeGreaterThanOrEqual(2.5 * 60);
    expect(run.readout().itemSlot).toEqual({
      ...EMPTY_SLOT,
      item: rolled,
      landing: true,
      boxesTaken: 1,
    });
    for (let tick = 0; tick < ITEM_LANDING_FLASH_S * 60; tick++) run.tick(IDLE_INPUT, DT);
    expect(run.readout().itemSlot).toEqual({ ...EMPTY_SLOT, item: rolled, boxesTaken: 1 });
  });

  it("counts a box taken into a full slot, which rolls nothing", async () => {
    const bridge = await loadBridge(wasm);
    const run = localRaceRun(bridge, "oval", boxesUnderTheGrid(2));
    started(run);
    run.tick(IDLE_INPUT, DT);
    const rolling = run.readout().itemSlot.rolling;
    // A kart takes one box a step: the second goes the step after Go.
    run.tick(IDLE_INPUT, DT);
    expect(run.readout().itemSlot.boxesTaken).toBe(2);
    expect(run.readout().itemSlot.rolling?.item).toBe(rolling?.item);
  });
});
