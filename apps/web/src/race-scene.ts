import type { Bridge, KartInput } from "./bridge";
import { createChaseCamera } from "./chase-camera";
import { gamepadKartInput } from "./gamepad";
import { combinedKartInput, IDLE_INPUT } from "./input";
import { createInstrumentPanel, type InstrumentPanel } from "./instrument-panel";
import { keyboardKartInput } from "./keyboard";
import { createRaceView } from "./race-view";
import { downloadReplay, recordReplay } from "./replay";
import type { RaceResults } from "./results-scene";
import type { TrackFiles } from "./track";

/**
 * Who is on the grid: the seed every client builds the sim from, how many
 * karts the humans take at the front of the grid, and which of them is the
 * local player's. A solo race is one human at kart 0 on a fixed seed; a room
 * hands out its own from the Countdown (ADR 0002).
 */
export interface RaceSetup {
  readonly seed: bigint;
  readonly humanCount: number;
  readonly localKart: number;
}

export const SOLO_RACE: RaceSetup = { seed: 1n, humanCount: 1, localKart: 0 };

/** Ticks run per frame at most, so a stalled tab catches up instead of spiralling. */
const MAX_TICKS_PER_FRAME = 5;
/** Downloads the session so far as a replay fixture (assets/replays/README.md). */
const DOWNLOAD_REPLAY_KEY = "r";

export interface RaceScene {
  /** The HUD over the View; the connection readout is written from outside the Scene. */
  readonly panel: InstrumentPanel;
  /** Settles when the sim reports the race finished, with the results. */
  readonly finished: Promise<RaceResults>;
  /** Stop stepping and drawing, and take the View and the panel off the page. */
  dispose(): void;
}

/**
 * The Race Scene: the sim stepped at the tuned rate from the keyboard and
 * the gamepad for the local kart, with bots driving the rest of the grid,
 * its one View drawn every frame from the chase camera, and the Instrument
 * panel over it. The sim runs the race from its countdown to its finish;
 * the Scene keeps drawing the finished race until it is disposed. The other
 * humans' karts hold idle until the server drives them (issue 17).
 */
export async function createRaceScene(
  container: HTMLElement,
  bridge: Bridge,
  track: TrackFiles,
  sidecar: string,
  setup: RaceSetup = SOLO_RACE,
): Promise<RaceScene> {
  if (setup.localKart < 0 || setup.localKart >= setup.humanCount) {
    throw new RangeError(`local kart ${setup.localKart} is not one of ${setup.humanCount} humans`);
  }
  const tickSeconds = 1 / bridge.ticksPerSecond;
  const botCount = bridge.kartsPerRace - setup.humanCount;
  const race = bridge.createRace(setup.seed, setup.humanCount, botCount, sidecar);
  const recorder = recordReplay(bridge, setup.seed, track.name, botCount, race);
  // Every listener the Scene adds is dropped together on dispose.
  const listeners = new AbortController();
  window.addEventListener(
    "keydown",
    (event) => {
      if (event.key === DOWNLOAD_REPLAY_KEY) {
        downloadReplay(recorder, document);
      }
    },
    { signal: listeners.signal },
  );
  const input = combinedKartInput(
    keyboardKartInput(window, listeners.signal),
    gamepadKartInput(navigator),
  );
  const view = await createRaceView(container, bridge.kartsPerRace, bridge.kartSize, track.gltfUrl);
  const camera = createChaseCamera();
  const panel = createInstrumentPanel(container, bridge.lapsPerRace);

  const present = (frameSeconds: number) => {
    const transforms = race.kartTransforms();
    const local = transforms[setup.localKart];
    const standing = race.standings()[setup.localKart];
    if (local && standing) {
      view.update(transforms, camera.update(local, frameSeconds));
      panel.update(local, standing, race.phase());
    }
  };
  present(0);

  let settle: (results: RaceResults) => void = () => {};
  const finished = new Promise<RaceResults>((resolve) => {
    settle = resolve;
  });
  const isBot = (slot: number): boolean => slot >= setup.humanCount;
  const inputs: KartInput[] = Array<KartInput>(setup.humanCount).fill(IDLE_INPUT);

  // Fixed-step loop: the sim ticks at its own rate however often frames come.
  let unsimulatedSeconds = 0;
  let last = performance.now();
  let frameHandle = 0;
  let over = false;
  const frame = (now: number) => {
    const frameSeconds = (now - last) / 1000;
    unsimulatedSeconds += Math.min(frameSeconds, MAX_TICKS_PER_FRAME * tickSeconds);
    last = now;
    while (!over && unsimulatedSeconds >= tickSeconds) {
      inputs[setup.localKart] = input.current();
      race.step(inputs, tickSeconds);
      recorder.record(inputs);
      unsimulatedSeconds -= tickSeconds;
      if (race.phase().kind === "finished") {
        over = true;
        settle({ standings: race.standings(), localKart: setup.localKart, isBot });
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
      view.dispose();
      panel.dispose();
    },
  };
}
