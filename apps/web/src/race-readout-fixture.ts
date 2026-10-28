import type { Standing } from "./bridge";
import { RESTING_KART } from "./kart-readout-fixture";
import type { RaceReadout } from "./race-run";

/** The standing of a kart first on lap 1 with no lap behind it. */
export const ON_THE_GRID: Standing = Object.freeze({
  position: 1,
  lap: 1,
  checkpoint: 0,
  splineProgress: 0,
  lapTimeS: 0,
  bestLapS: null,
  wrongWay: false,
  finishTimeS: null,
});

/**
 * A race for a test to vary, built on the run's one readout type: the
 * resting kart alone on the grid, well into the race with an empty item
 * slot, and nothing on the track. A field the run adds to the readout gets
 * its default here and nowhere else.
 */
export function raceReadout(overrides: Partial<RaceReadout> = {}): RaceReadout {
  return {
    karts: [RESTING_KART],
    standings: [ON_THE_GRID],
    phase: { kind: "racing", elapsedSeconds: 10, capLeftSeconds: null },
    itemSlot: { item: null, firing: false },
    itemBoxesPresent: [],
    hazards: [],
    projectiles: [],
    wheels: [],
    ...overrides,
  };
}
