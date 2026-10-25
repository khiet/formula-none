import type { RaceAudio } from "./audio";
import { soundReading } from "./audio-cues";
import type {
  Bridge,
  Hazard,
  ItemBox,
  KartInput,
  KartTransform,
  Phase,
  Projectile,
  Standing,
} from "./bridge";
import { createChaseCamera } from "./chase-camera";
import { gamepadKartInput } from "./gamepad";
import { combinedKartInput, IDLE_INPUT } from "./input";
import {
  createInstrumentPanel,
  type InstrumentPanel,
  type ItemSlot,
  loadInstrumentFont,
} from "./instrument-panel";
import { kartLabels } from "./kart-labels";
import { KART_GLTF_URL } from "./kart-model";
import { keyboardKartInput } from "./keyboard";
import { createRaceView } from "./race-view";
import { downloadReplay, type ReplayRecorder, recordReplay } from "./replay";
import { kartName, type RaceResults } from "./results-scene";
import type { TrackFiles, TrackName, WorldPoint } from "./track";

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
  /** The local kart's item slot, as of the last tick. */
  itemSlot(): ItemSlot;
  /** The track's item boxes, in the sidecar's order; fixed for the race. */
  itemBoxes(): readonly ItemBox[];
  /** Whether each item box, in the same order, is there to pick up, as of the last tick. */
  itemBoxesPresent(): readonly boolean[];
  /** The hazards on the track where the View draws them, as of the last tick. */
  hazards(): readonly Hazard[];
  /** The projectiles in flight where the View draws them, as of the last tick. */
  projectiles(): readonly Projectile[];
  /** The run as a replay fixture, when the rules ran here to record. */
  readonly replay?: ReplayRecorder;
  /** Let go of whatever the run follows; called when the Scene is disposed. */
  dispose?(): void;
}

/** Ticks run per frame at most, so a stalled tab catches up instead of spiralling. */
const MAX_TICKS_PER_FRAME = 5;
/** How long the item slot shows the fire after the item leaves it. Presentation, so not the sim's to tune. */
export const ITEM_FIRE_FLASH_S = 0.5;
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
  const boxes = race.itemBoxes();
  const heldItem = () => race.kartTransforms()[setup.localKart]?.heldItem ?? null;
  // The only way an item leaves the slot here is the fire, which the sim
  // applies the tick it is pressed; the slot shows it for the flash.
  let flashTicksLeft = 0;
  return {
    humanCount: setup.humanCount,
    localKart: setup.localKart,
    tick(input, dt) {
      inputs[setup.localKart] = input;
      const before = heldItem();
      race.step(inputs, dt);
      replay.record(inputs);
      flashTicksLeft =
        before !== null && heldItem() === null
          ? Math.round(ITEM_FIRE_FLASH_S * bridge.ticksPerSecond)
          : Math.max(flashTicksLeft - 1, 0);
    },
    kartTransforms: () => race.kartTransforms(),
    standings: () => race.standings(),
    phase: () => race.phase(),
    itemSlot: () => ({ item: heldItem(), firing: flashTicksLeft > 0 }),
    itemBoxes: () => boxes,
    itemBoxesPresent: () => race.itemBoxesPresent(),
    hazards: () => race.hazards(),
    projectiles: () => race.projectiles(),
    replay,
  };
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
  // The panel's typeface loads beside the View's models so the first frame
  // of the Scene is set in it.
  const [view] = await Promise.all([
    createRaceView(
      container,
      bridge.kartsPerRace,
      bridge.kartSize,
      { trackGltfUrl: track.gltfUrl, kartGltfUrl: KART_GLTF_URL },
      run.itemBoxes(),
      bridge.spinOutSeconds,
    ),
    loadInstrumentFont(),
  ]);
  const camera = createChaseCamera(bridge.cameraTuning);
  const panel = createInstrumentPanel(container, bridge.lapsPerRace, {
    centreline,
    kartCount: bridge.kartsPerRace,
    ownKart: run.localKart,
  });

  const isBot = (kart: number): boolean => kart >= run.humanCount;
  const name = (kart: number): string =>
    kartName({ localKart: run.localKart, isBot, nickname }, kart);
  const present = (frameSeconds: number) => {
    const transforms = run.kartTransforms();
    const local = transforms[run.localKart];
    const standing = run.standings()[run.localKart];
    if (local && standing) {
      const phase = run.phase();
      view.update(
        transforms,
        kartLabels(transforms, run.localKart, name),
        camera.update(local, frameSeconds),
        run.itemBoxesPresent(),
        run.hazards(),
        run.projectiles(),
        phase,
        frameSeconds,
      );
      const slot = run.itemSlot();
      panel.update(local, standing, phase, slot, transforms);
      audio.update(soundReading(local, standing, slot, phase));
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
      audio.stop();
      run.dispose?.();
      view.dispose();
      panel.dispose();
    },
  };
}
