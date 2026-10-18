import { loadBridge } from "./bridge";
import { createInstrumentPanel } from "./instrument-panel";
import { keyboardKartInput } from "./keyboard";
import { createRaceView } from "./race-view";

// M0: one kart, one plane, the local player at slot 0. The seed is fixed
// until rooms hand one out.
const SEED = 1n;
const KART_COUNT = 1;
const LOCAL_KART = 0;
/** Ticks run per frame at most, so a stalled tab catches up instead of spiralling. */
const MAX_TICKS_PER_FRAME = 5;

async function main(container: HTMLElement): Promise<void> {
  const bridge = await loadBridge();
  const tickSeconds = 1 / bridge.ticksPerSecond;
  const race = bridge.createRace(SEED, KART_COUNT);
  const input = keyboardKartInput(window);
  const view = await createRaceView(container, KART_COUNT);
  const panel = createInstrumentPanel(container);

  let transforms = race.kartTransforms();
  view.update(transforms, LOCAL_KART);

  // Fixed-step loop: the sim ticks at its own rate however often frames come.
  let carry = 0;
  let last = performance.now();
  const frame = (now: number) => {
    carry += Math.min((now - last) / 1000, MAX_TICKS_PER_FRAME * tickSeconds);
    last = now;
    while (carry >= tickSeconds) {
      const before = transforms[LOCAL_KART];
      race.step([input.current()], tickSeconds);
      transforms = race.kartTransforms();
      const after = transforms[LOCAL_KART];
      if (after) {
        panel.update(before, after, tickSeconds);
      }
      carry -= tickSeconds;
    }
    view.update(transforms, LOCAL_KART);
    view.render();
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

const app = document.querySelector<HTMLDivElement>("#app");
if (app) {
  main(app).catch((error: unknown) => {
    console.error("Formula None failed to start", error);
  });
}
