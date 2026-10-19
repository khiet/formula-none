import type { Bridge, KartInput, KartTransform, Phase, Standing } from "./bridge";
import { createChaseCamera } from "./chase-camera";
import { gamepadKartInput } from "./gamepad";
import { combinedKartInput, IDLE_INPUT } from "./input";
import { createInstrumentPanel, type InstrumentPanel } from "./instrument-panel";
import { keyboardKartInput } from "./keyboard";
import { createRaceView } from "./race-view";
import { downloadReplay, type ReplayRecorder, recordReplay } from "./replay";
import type { RaceResults } from "./results-scene";
import type { TrackFiles, TrackName } from "./track";

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
 * A race as the Scene runs and reads it, wherever the rules run: the sim in
 * this browser for a solo race, or the server's snapshots for a room's
 * (ADR 0009). The Scene ticks it with the local player's input and draws
 * what it reports.
 */
export interface RaceRun {
  readonly humanCount: number;
  readonly localKart: number;
  /** One tick of `dt` seconds with the local player's input: stepped here, or sent to the server. */
  tick(input: KartInput, dt: number): void;
  /** One transform per kart in grid order, as of the last tick. */
  kartTransforms(): KartTransform[];
  /** One standing per kart in grid order, as of the last tick. */
  standings(): Standing[];
  phase(): Phase;
  /** The run as a replay fixture, when the rules ran here to record. */
  readonly replay?: ReplayRecorder;
  /** Let go of whatever the run follows; called when the Scene is disposed. */
  dispose?(): void;
}

/** Ticks run per frame at most, so a stalled tab catches up instead of spiralling. */
const MAX_TICKS_PER_FRAME = 5;
/** Downloads the session so far as a replay fixture (assets/replays/README.md). */
const DOWNLOAD_REPLAY_KEY = "r";

export interface RaceScene {
  /** The HUD over the View; the connection readout is written from outside the Scene. */
  readonly panel: InstrumentPanel;
  /** Settles when the run reports the race finished, with the results. */
  readonly finished: Promise<RaceResults>;
  /** Stop stepping and drawing, and take the View and the panel off the page. */
  dispose(): void;
}

/**
 * A race whose rules run in this browser: the sim built from `setup` on the
 * track `sidecar` describes, the local kart driven by the Scene's input and
 * the other humans' karts held idle, with bots on the rest of the grid, and
 * every tick recorded as a replay. Throws when the local kart is not one of
 * the humans.
 */
export function localRaceRun(
  bridge: Bridge,
  track: TrackName,
  sidecar: string,
  setup: RaceSetup = SOLO_RACE,
): RaceRun {
  if (setup.localKart < 0 || setup.localKart >= setup.humanCount) {
    throw new RangeError(`local kart ${setup.localKart} is not one of ${setup.humanCount} humans`);
  }
  const botCount = bridge.kartsPerRace - setup.humanCount;
  const race = bridge.createRace(setup.seed, setup.humanCount, botCount, sidecar);
  const replay = recordReplay(bridge, setup.seed, track, botCount, race);
  const inputs: KartInput[] = Array<KartInput>(setup.humanCount).fill(IDLE_INPUT);
  return {
    humanCount: setup.humanCount,
    localKart: setup.localKart,
    tick(input, dt) {
      inputs[setup.localKart] = input;
      race.step(inputs, dt);
      replay.record(inputs);
    },
    kartTransforms: () => race.kartTransforms(),
    standings: () => race.standings(),
    phase: () => race.phase(),
    replay,
  };
}

/**
 * The Race Scene: the run ticked at the tuned rate from the keyboard and
 * the gamepad for the local kart, its one View drawn every frame from the
 * chase camera, and the Instrument panel over it. The run carries the race
 * from its countdown to its finish; the Scene keeps drawing the finished
 * race until it is disposed.
 */
export async function createRaceScene(
  container: HTMLElement,
  bridge: Bridge,
  track: TrackFiles,
  run: RaceRun,
): Promise<RaceScene> {
  const tickSeconds = 1 / bridge.ticksPerSecond;
  // Every listener the Scene adds is dropped together on dispose.
  const listeners = new AbortController();
  const { replay } = run;
  if (replay !== undefined) {
    window.addEventListener(
      "keydown",
      (event) => {
        if (event.key === DOWNLOAD_REPLAY_KEY) {
          downloadReplay(replay, document);
        }
      },
      { signal: listeners.signal },
    );
  }
  const input = combinedKartInput(
    keyboardKartInput(window, listeners.signal),
    gamepadKartInput(navigator),
  );
  const view = await createRaceView(container, bridge.kartsPerRace, bridge.kartSize, track.gltfUrl);
  const camera = createChaseCamera();
  const panel = createInstrumentPanel(container, bridge.lapsPerRace);

  const present = (frameSeconds: number) => {
    const transforms = run.kartTransforms();
    const local = transforms[run.localKart];
    const standing = run.standings()[run.localKart];
    if (local && standing) {
      view.update(transforms, camera.update(local, frameSeconds));
      panel.update(local, standing, run.phase());
    }
  };
  present(0);

  let settle: (results: RaceResults) => void = () => {};
  const finished = new Promise<RaceResults>((resolve) => {
    settle = resolve;
  });
  const isBot = (kart: number): boolean => kart >= run.humanCount;

  // Fixed-step loop: the run ticks at the sim's rate however often frames come.
  let unsimulatedSeconds = 0;
  let last = performance.now();
  let frameHandle = 0;
  let over = false;
  const frame = (now: number) => {
    const frameSeconds = (now - last) / 1000;
    unsimulatedSeconds += Math.min(frameSeconds, MAX_TICKS_PER_FRAME * tickSeconds);
    last = now;
    while (!over && unsimulatedSeconds >= tickSeconds) {
      run.tick(input.current(), tickSeconds);
      unsimulatedSeconds -= tickSeconds;
      if (run.phase().kind === "finished") {
        over = true;
        settle({ standings: run.standings(), localKart: run.localKart, isBot });
      }
    }
    present(frameSeconds);
    view.render();
    frameHandle = requestAnimationFrame(frame);
  };
  frameHandle = requestAnimationFrame(frame);

  return {
    panel,
    finished,
    dispose() {
      cancelAnimationFrame(frameHandle);
      listeners.abort();
      run.dispose?.();
      view.dispose();
      panel.dispose();
    },
  };
}
