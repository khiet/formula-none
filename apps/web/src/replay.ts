// Recording a browser session as a replay fixture (ADR 0002). The file
// format is the sim's: crates/sim/src/replay.rs documents it and the native
// and wasm runners both read it. Drop a recording into assets/replays and
// both halves of the determinism gate pick it up.
import type { BotLevel, Bridge, KartInput, Race, Tyres } from "./bridge";
import type { TrackName } from "./track";

export interface ReplayRecorder {
  readonly track: TrackName;
  /** Call once per tick, after `race` stepped with the humans' `inputs`. */
  record(inputs: readonly KartInput[]): void;
  /** The session so far as a replay fixture, pretty-printed like the committed ones. */
  toJSON(): string;
}

/** The fixture file's shape; the sim reads the seed and the hashes as u64. */
export interface ReplayFile {
  readonly version: 1;
  /** Decimal, as a string: a JSON number cannot carry every u64. */
  readonly seed: string;
  readonly track: string;
  /**
   * Bot karts behind the humans; they drive themselves from the seed, so the
   * segments carry no input for them.
   */
  readonly bots: number;
  /** How hard the bots drove (ADR 0078); a replay without one reruns at Basic. */
  readonly level: BotLevel;
  /** Whether the tyres wore (ADR 0083); a replay without a pick reruns Off. */
  readonly tyres: Tyres;
  readonly segments: readonly Segment[];
  /** 16 hex digits each. */
  readonly hashes: readonly string[];
}

export interface Segment {
  readonly ticks: number;
  readonly inputs: readonly KartInput[];
}

/**
 * Record `race`, which was created from `seed` on `track` with `bots` bot
 * karts at `level` and the tyres as `tyres` says, from its first tick. Consecutive ticks with the same inputs share a
 * segment, and the race's hash is kept at every
 * `bridge.replayHashIntervalTicks`th tick.
 */
export function recordReplay(
  bridge: Bridge,
  seed: bigint,
  track: TrackName,
  bots: number,
  level: BotLevel,
  tyres: Tyres,
  race: Race,
): ReplayRecorder {
  const segments: { ticks: number; inputs: KartInput[] }[] = [];
  const hashes: string[] = [];
  let ticks = 0;
  return {
    track,
    record(inputs) {
      const copy = inputs.map(({ steer, throttle, brake, hop, useItem, holdItem }) => ({
        steer,
        throttle,
        brake,
        hop,
        useItem,
        holdItem,
      }));
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
      const file: ReplayFile = {
        version: 1,
        seed: seed.toString(),
        track,
        bots,
        level,
        tyres,
        segments,
        hashes,
      };
      return `${JSON.stringify(file, null, 2)}\n`;
    },
  };
}

function sameInputs(a: readonly KartInput[], b: readonly KartInput[]): boolean {
  return (
    a.length === b.length &&
    a.every((input, slot) => {
      const other = b[slot];
      return (
        other !== undefined &&
        input.steer === other.steer &&
        input.throttle === other.throttle &&
        input.brake === other.brake &&
        input.hop === other.hop &&
        input.useItem === other.useItem &&
        input.holdItem === other.holdItem
      );
    })
  );
}

/** Hand the recording to the browser as a download named after the track. */
export function downloadReplay(recorder: ReplayRecorder, document: Document): void {
  const url = URL.createObjectURL(new Blob([recorder.toJSON()], { type: "application/json" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `${recorder.track}.replay.json`;
  anchor.click();
  URL.revokeObjectURL(url);
}
