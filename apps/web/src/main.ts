import { type Bridge, loadBridge } from "./bridge";
import { connect, serverUrls } from "./connection";
import type { InstrumentPanel } from "./instrument-panel";
import { ping } from "./ping";
import { createRaceScene } from "./race-scene";
import { fetchSidecar, selectTrack } from "./track";

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

// M0: straight into the Race Scene; the track comes from `?track=` until
// rooms pick it.
async function main(container: HTMLElement): Promise<void> {
  const track = selectTrack(window.location.search);
  const [bridge, sidecar] = await Promise.all([loadBridge(), fetchSidecar(track)]);
  const scene = await createRaceScene(container, bridge, track, sidecar);
  void reportConnection(bridge, scene.panel);
}

const app = document.querySelector<HTMLDivElement>("#app");
if (app) {
  main(app).catch((error: unknown) => {
    console.error("Formula None failed to start", error);
  });
}
