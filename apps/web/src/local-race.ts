import type { BotLevel, Bridge, KartInput } from "./bridge";
import { IDLE_INPUT } from "./input";
import {
  blastLedger,
  ITEM_FIRE_FLASH_S,
  ITEM_LANDING_FLASH_S,
  landedItem,
  type RaceRun,
  rollingOf,
} from "./race-run";
import { recordReplay } from "./replay";
import type { TrackName } from "./track";

/**
 * Who is on the grid of a local race: the seed the sim is built from, how
 * many karts the humans take at the front of the grid, and which of them is
 * the local player's. A solo race is one human at kart 0 on a fixed seed.
 */
export interface RaceSetup {
  readonly seed: bigint;
  readonly humanCount: number;
  readonly localKart: number;
}

export const SOLO_RACE: RaceSetup = { seed: 1n, humanCount: 1, localKart: 0 };

/**
 * A race whose rules run in this browser: the sim built from `setup` on the
 * track `sidecar` describes, the local kart driven by the Scene's input and
 * the other humans' karts held idle, with bots at `botLevel` on the rest of
 * the grid, and every tick recorded as a replay. Throws when the local kart is not one of
 * the humans.
 */
export function localRaceRun(
  bridge: Bridge,
  track: TrackName,
  sidecar: string,
  botLevel: BotLevel,
  setup: RaceSetup = SOLO_RACE,
): RaceRun {
  if (setup.localKart < 0 || setup.localKart >= setup.humanCount) {
    throw new RangeError(`local kart ${setup.localKart} is not one of ${setup.humanCount} humans`);
  }
  const botCount = bridge.kartsPerRace - setup.humanCount;
  const race = bridge.createRace(setup.seed, setup.humanCount, botCount, sidecar, botLevel);
  const replay = recordReplay(bridge, setup.seed, track, botCount, botLevel, race);
  const inputs: KartInput[] = Array<KartInput>(setup.humanCount).fill(IDLE_INPUT);
  const boxes = race.itemBoxes();
  const coins = race.coins();
  const slot = () => race.kartReadouts()[setup.localKart]?.itemSlot ?? "empty";
  const ticks = (seconds: number) => Math.round(seconds * bridge.ticksPerSecond);
  let boxesTaken = 0;
  // The only way an item leaves the slot here is the fire, which the sim
  // applies the tick it is pressed; the slot shows it for the flash.
  let flashTicksLeft = 0;
  let landingTicksLeft = 0;
  const blasts = blastLedger(bridge.ticksPerSecond);
  return {
    humanCount: setup.humanCount,
    localKart: setup.localKart,
    tick(input, dt) {
      inputs[setup.localKart] = input;
      const before = slot();
      race.step(inputs, dt);
      replay.record(inputs);
      boxesTaken += race.boxesTakenLastStep(setup.localKart);
      const after = slot();
      flashTicksLeft =
        landedItem(before) !== null && landedItem(after) === null
          ? ticks(ITEM_FIRE_FLASH_S)
          : Math.max(flashTicksLeft - 1, 0);
      landingTicksLeft =
        rollingOf(before, bridge.itemRollSeconds) !== null && landedItem(after) !== null
          ? ticks(ITEM_LANDING_FLASH_S)
          : Math.max(landingTicksLeft - 1, 0);
      blasts.tick();
      // A blast is where the sim has the kart the tick it fired.
      const karts = race.kartReadouts();
      for (const kart of race.hornsFiredLastStep()) {
        const position = karts[kart]?.position;
        if (position !== undefined) blasts.fired(kart, position);
      }
    },
    readout: () => {
      const karts = race.kartReadouts();
      const own = karts[setup.localKart]?.itemSlot ?? "empty";
      return {
        karts,
        standings: race.standings(),
        phase: race.phase(),
        itemSlot: {
          item: landedItem(own),
          rolling: rollingOf(own, bridge.itemRollSeconds),
          firing: flashTicksLeft > 0,
          landing: landingTicksLeft > 0,
          boxesTaken,
        },
        itemBoxesPresent: race.itemBoxesPresent(),
        coinsPresent: race.coinsPresent(),
        hazards: race.hazards(),
        projectiles: race.projectiles(),
        wheels: race.wheels(),
        blasts: blasts.blasts(),
      };
    },
    itemBoxes: () => boxes,
    coins: () => coins,
    replay,
  };
}
