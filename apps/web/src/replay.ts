// Recording a browser session as a replay fixture (ADR 0002). The file
// format is the sim's: crates/sim/src/replay.rs documents it and the native
// and wasm runners both read it. Drop a recording into assets/replays and
// both halves of the determinism gate pick it up.
import type { Bridge, KartInput, Race } from "./bridge";
import type { TrackName } from "./track";

export interface ReplayRecorder {
  /** Call once per tick, after `race` stepped with `inputs`. */
  recorded(inputs: readonly KartInput[]): void;
  /** The session so far as a replay fixture, pretty-printed like the committed ones. */
  toJSON(): string;
}

interface Segment {
  ticks: number;
  inputs: KartInput[];
}

/**
 * Record `race`, which was created from `seed` on `track`, from its first
 * tick. Consecutive ticks with the same inputs share a segment, and the
 * race's hash is kept at every `bridge.replayHashIntervalTicks`th tick.
 */
export function recordReplay(
  bridge: Bridge,
  seed: bigint,
  track: TrackName,
  race: Race,
): ReplayRecorder {
  const segments: Segment[] = [];
  const hashes: string[] = [];
  let ticks = 0;
  return {
    recorded(inputs) {
      const copy = inputs.map(({ steer, throttle, brake }) => ({ steer, throttle, brake }));
      const last = segments.at(-1);
      if (last !== undefined && sameInputs(last.inputs, copy)) {
        last.ticks += 1;
      } else {
        segments.push({ ticks: 1, inputs: copy });
      }
      ticks += 1;
      if (ticks % bridge.replayHashIntervalTicks === 0) {
        hashes.push(race.stateHash().toString(16).padStart(16, "0"));
      }
    },
    toJSON() {
      // The seed is a JSON number: it fits while rooms hand out small seeds,
      // and the sim reads it as u64 either way.
      return `${JSON.stringify({ version: 1, seed: Number(seed), track, segments, hashes }, null, 2)}\n`;
    },
  };
}

function sameInputs(a: readonly KartInput[], b: readonly KartInput[]): boolean {
  return (
    a.length === b.length &&
    a.every(
      (input, slot) =>
        input.steer === b[slot]?.steer &&
        input.throttle === b[slot]?.throttle &&
        input.brake === b[slot]?.brake,
    )
  );
}

/** Hand the recording to the browser as a download named after the track. */
export function downloadReplay(
  recorder: ReplayRecorder,
  track: TrackName,
  document: Document,
): void {
  const url = URL.createObjectURL(new Blob([recorder.toJSON()], { type: "application/json" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `${track}.replay.json`;
  anchor.click();
  URL.revokeObjectURL(url);
}
