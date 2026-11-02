import type { RaceAudio } from "./audio";
import { soundReading } from "./audio-cues";
import type { Bridge } from "./bridge";
import { createChaseCamera } from "./chase-camera";
import { gamepadKartInput } from "./gamepad";
import { combinedKartInput } from "./input";
import {
  createInstrumentPanel,
  type InstrumentPanel,
  loadInstrumentFont,
} from "./instrument-panel";
import { kartLabels } from "./kart-labels";
import { KART_GLTF_URL } from "./kart-model";
import { keyboardKartInput } from "./keyboard";
import { livery } from "./livery";
import type { RaceRun } from "./race-run";
import { createRaceView } from "./race-view";
import { downloadReplay } from "./replay";
import { kartName, type RaceResults } from "./results-scene";
import type { TrackFiles, WorldPoint } from "./track";

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
 * The Race Scene: the run ticked at the tuned rate from the keyboard and
 * the gamepad for the local kart, its one View drawn every frame from the
 * chase camera with every other kart named over it, by `nickname` for the
 * humans the room named, and the Instrument panel over it with the course
 * map drawn from `centreline`. `audio` sounds the race from the own kart's
 * readings and is stopped with the Scene. The run carries the race from its
 * countdown to its finish; the Scene keeps drawing the finished race until
 * it is disposed.
 */
export async function createRaceScene(
  container: HTMLElement,
  bridge: Bridge,
  track: TrackFiles,
  centreline: readonly WorldPoint[],
  run: RaceRun,
  nickname: (kart: number) => string | undefined,
  audio: RaceAudio,
): Promise<RaceScene> {
  const tickSeconds = 1 / bridge.ticksPerSecond;
  const isBot = (kart: number): boolean => kart >= run.humanCount;
  const name = (kart: number): string =>
    kartName({ localKart: run.localKart, isBot, nickname }, kart);
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
  // Read once per race: the kick and the wind lines follow the viewer's
  // setting as the race begins.
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  // The panel's typeface loads beside the View's models so the first frame
  // of the Scene is set in it.
  const [view] = await Promise.all([
    createRaceView(
      container,
      bridge.kartsPerRace,
      bridge.kartSize,
      { trackGltfUrl: track.gltfUrl, kartGltfUrl: KART_GLTF_URL },
      run.itemBoxes(),
      run.coins(),
      bridge.spinOutSeconds,
      bridge.runawayWheelRadiusMetres,
      bridge.hornRadiusMetres,
      (karts) => kartLabels(karts, run.localKart, name),
      { localKart: run.localKart, reducedMotion },
    ),
    loadInstrumentFont(),
  ]);
  const camera = createChaseCamera(bridge.cameraTuning, { reducedMotion });
  const panel = createInstrumentPanel(
    container,
    bridge.lapsPerRace,
    { centreline, kartCount: bridge.kartsPerRace, ownKart: run.localKart },
    {
      coinTopSpeeds: bridge.coinTopSpeeds,
      coinCap: bridge.coinCap,
      starSeconds: bridge.starSeconds,
    },
    name,
    livery(run.localKart),
    { reducedMotion },
  );

  const present = (frameSeconds: number) => {
    const readout = run.readout();
    const local = readout.karts[run.localKart];
    if (local && readout.standings[run.localKart]) {
      const pose = camera.update(local, frameSeconds, input.lookingBehind());
      view.update(readout, pose, frameSeconds);
      panel.update(readout);
      audio.update(soundReading(readout, run.localKart));
    }
  };
  audio.start();
  present(0);

  let settle: (results: RaceResults) => void = () => {};
  const finished = new Promise<RaceResults>((resolve) => {
    settle = resolve;
  });

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
      const { phase, standings } = run.readout();
      if (phase.kind === "finished") {
        over = true;
        settle({ standings, localKart: run.localKart, isBot });
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
      audio.stop();
      run.dispose?.();
      view.dispose();
      panel.dispose();
    },
  };
}
