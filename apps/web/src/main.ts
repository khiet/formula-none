import { type Bridge, loadBridge } from "./bridge";
import { connect, serverUrls } from "./connection";
import { createInstrumentPanel, type InstrumentPanel } from "./instrument-panel";
import { keyboardKartInput } from "./keyboard";
import { ping } from "./ping";
import { createRaceView } from "./race-view";
import { downloadReplay, recordReplay } from "./replay";
import { fetchSidecar, selectTrack } from "./track";

// M0: one kart on a greybox track, the local player at slot 0. The seed is
// fixed until rooms hand one out; the track comes from `?track=` until rooms
// pick it.
const SEED = 1n;
const KART_COUNT = 1;
const LOCAL_KART = 0;
/** Ticks run per frame at most, so a stalled tab catches up instead of spiralling. */
const MAX_TICKS_PER_FRAME = 5;
/** Downloads the session so far as a replay fixture (assets/replays/README.md). */
const DOWNLOAD_REPLAY_KEY = "r";

/**
 * M0(b): reach the server, WebTransport first and WebSocket as the fallback,
 * and show which answered and how fast. The race runs locally either way, so
 * no failure here is fatal.
 */
async function reportConnection(bridge: Bridge, panel: InstrumentPanel): Promise<void> {
  try {
    const connection = await connect({
      ...serverUrls(window.location),
      webTransportTimeoutMs: bridge.webTransportTimeoutMs,
    });
    const nonce = Math.floor(Math.random() * 0x1_0000_0000);
    const rtt = await ping(connection, bridge, nonce, bridge.pingTiming);
    panel.connection(`${connection.transport} ${Math.round(rtt)} ms`);
    connection.closed.then(() => panel.connection("offline"));
  } catch (error) {
    console.warn("no connection to the server", error);
    panel.connection("offline");
  }
}

async function main(container: HTMLElement): Promise<void> {
  const track = selectTrack(window.location.search);
  const [bridge, sidecar] = await Promise.all([loadBridge(), fetchSidecar(track)]);
  const tickSeconds = 1 / bridge.ticksPerSecond;
  const race = bridge.createRace(SEED, KART_COUNT, sidecar);
  const recorder = recordReplay(bridge, SEED, track.name, race);
  window.addEventListener("keydown", (event) => {
    if (event.key === DOWNLOAD_REPLAY_KEY) {
      downloadReplay(recorder, document);
    }
  });
  const input = keyboardKartInput(window);
  const view = await createRaceView(container, KART_COUNT, track.gltfUrl);
  const panel = createInstrumentPanel(container);
  void reportConnection(bridge, panel);

  const present = () => {
    const transforms = race.kartTransforms();
    view.update(transforms, LOCAL_KART);
    const local = transforms[LOCAL_KART];
    if (local) {
      panel.update(local);
    }
  };
  present();

  // Fixed-step loop: the sim ticks at its own rate however often frames come.
  let unsimulatedSeconds = 0;
  let last = performance.now();
  const frame = (now: number) => {
    unsimulatedSeconds += Math.min((now - last) / 1000, MAX_TICKS_PER_FRAME * tickSeconds);
    last = now;
    while (unsimulatedSeconds >= tickSeconds) {
      const inputs = [input.current()];
      race.step(inputs, tickSeconds);
      recorder.record(inputs);
      unsimulatedSeconds -= tickSeconds;
    }
    present();
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
