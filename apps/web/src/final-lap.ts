// The final-lap set piece (ADR 0100): from the first tick of the own kart's
// last lap to its finish, the gantry reads FINAL LAP, fireworks go up over
// the course's landmark, the crowd comes up all round the lap and the race
// music plays faster and louder. Presentation only, from the readout and the
// picked laps: nothing goes on the wire, and each client lifts for its own
// kart.

import type { LoopPlayback } from "./audio-cues";
import type { PresentationBlend } from "./presentation-zones";
import type { RaceReadout } from "./race-run";

/**
 * Whether `kart` is on the race's last of `laps` while the race runs: off
 * before it, on from its first tick and off again once the kart has
 * finished, though the standing's lap stays on the last one.
 */
export function finalLap(race: RaceReadout, kart: number, laps: number): boolean {
  const standing = race.standings[kart];
  return (
    standing !== undefined &&
    race.phase.kind === "racing" &&
    standing.finishTimeS === null &&
    standing.lap === laps
  );
}

/** The crowd's level all round the final lap: louder than any stand's zone. */
const FINAL_LAP_CROWD = 1;

/** `blend` with the crowd up at its whole level through the final lap; untouched off it. */
export function finalLapBlend(blend: PresentationBlend, on: boolean): PresentationBlend {
  return on ? { ...blend, crowd: Math.max(blend.crowd, FINAL_LAP_CROWD) } : blend;
}

/**
 * The race music's lift on the final lap: a tempo change rather than a stem
 * of its own, so the loop keeps its place and needs no second file: 8%
 * faster, a semitone and a third up, and louder over the crowd.
 */
const FINAL_LAP_MUSIC: LoopPlayback = { rate: 1.08, gain: 1.3 };
const RACE_MUSIC: LoopPlayback = { rate: 1, gain: 1 };

/** How the race music plays: as rendered, or lifted on the final lap. */
export function musicPlayback(on: boolean): LoopPlayback {
  return on ? FINAL_LAP_MUSIC : RACE_MUSIC;
}
