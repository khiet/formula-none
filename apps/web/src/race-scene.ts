import type { Bridge } from "./bridge";
import { createChaseCamera } from "./chase-camera";
import { gamepadKartInput } from "./gamepad";
import { combinedKartInput } from "./input";
import { createInstrumentPanel, type InstrumentPanel } from "./instrument-panel";
import { keyboardKartInput } from "./keyboard";
import { createRaceView } from "./race-view";
import { downloadReplay, recordReplay } from "./replay";
import type { TrackFiles } from "./track";

// Until rooms hand out the players and the seed: one local human at slot 0,
// bots on the rest of the grid, and a fixed seed.
const SEED = 1n;
const HUMAN_COUNT = 1;
const LOCAL_KART = 0;
/** Ticks run per frame at most, so a stalled tab catches up instead of spiralling. */
const MAX_TICKS_PER_FRAME = 5;
/** Downloads the session so far as a replay fixture (assets/replays/README.md). */
const DOWNLOAD_REPLAY_KEY = "r";

export interface RaceScene {
  /** The HUD over the View; the connection readout is written from outside the Scene. */
  readonly panel: InstrumentPanel;
}

/**
 * The Race Scene: the sim stepped at the tuned rate from the keyboard and
 * the gamepad for the local kart, with bots driving the rest of the grid,
 * its one View drawn every frame from the chase camera, and the Instrument
 * panel over it. Runs until the page goes away.
 */
export async function createRaceScene(
  container: HTMLElement,
  bridge: Bridge,
  track: TrackFiles,
  sidecar: string,
): Promise<RaceScene> {
  const tickSeconds = 1 / bridge.ticksPerSecond;
  const botCount = bridge.kartsPerRace - HUMAN_COUNT;
  const race = bridge.createRace(SEED, HUMAN_COUNT, botCount, sidecar);
  const recorder = recordReplay(bridge, SEED, track.name, botCount, race);
  window.addEventListener("keydown", (event) => {
    if (event.key === DOWNLOAD_REPLAY_KEY) {
      downloadReplay(recorder, document);
    }
  });
  const input = combinedKartInput(keyboardKartInput(window), gamepadKartInput(navigator));
  const view = await createRaceView(container, bridge.kartsPerRace, bridge.kartSize, track.gltfUrl);
  const camera = createChaseCamera();
  const panel = createInstrumentPanel(container, bridge.lapsPerRace);

  const present = (frameSeconds: number) => {
    const transforms = race.kartTransforms();
    const local = transforms[LOCAL_KART];
    const standing = race.standings()[LOCAL_KART];
    if (local && standing) {
      view.update(transforms, camera.update(local, frameSeconds));
      panel.update(local, standing);
    }
  };
  present(0);

  // Fixed-step loop: the sim ticks at its own rate however often frames come.
  let unsimulatedSeconds = 0;
  let last = performance.now();
  const frame = (now: number) => {
    const frameSeconds = (now - last) / 1000;
    unsimulatedSeconds += Math.min(frameSeconds, MAX_TICKS_PER_FRAME * tickSeconds);
    last = now;
    while (unsimulatedSeconds >= tickSeconds) {
      const inputs = [input.current()];
      race.step(inputs, tickSeconds);
      recorder.record(inputs);
      unsimulatedSeconds -= tickSeconds;
    }
    present(frameSeconds);
    view.render();
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);

  return { panel };
}
