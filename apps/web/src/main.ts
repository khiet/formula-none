import { type Bridge, loadBridge } from "./bridge";
import { connect, serverUrls } from "./connection";
import type { InstrumentPanel } from "./instrument-panel";
import { ping } from "./ping";
import { createRaceScene } from "./race-scene";
import { createResultsScene, resultsRows } from "./results-scene";
import { createTitleScene } from "./title-scene";
import { fetchSidecar, selectTrack } from "./track";

/** The connection readout, kept across Race Scenes: each new panel shows the latest status. */
interface ConnectionReadout {
  show(panel: InstrumentPanel): void;
}

/**
 * M0(b): reach the server, WebTransport first and WebSocket as the fallback,
 * and show which answered and how fast. The race runs locally either way, so
 * no failure here is fatal.
 */
function reportConnection(bridge: Bridge): ConnectionReadout {
  let status = "connecting";
  let panel: InstrumentPanel | undefined;
  const report = (next: string) => {
    status = next;
    panel?.connection(status);
  };
  (async () => {
    try {
      const connection = await connect({
        ...serverUrls(window.location),
        webTransportTimeoutMs: bridge.webTransportTimeoutMs,
      });
      const nonce = Math.floor(Math.random() * 0x1_0000_0000);
      const rtt = await ping(connection, bridge, nonce, bridge.pingTiming);
      report(`${connection.transport} ${Math.round(rtt)} ms`);
      connection.closed.then(() => report("offline"));
    } catch (error) {
      console.warn("no connection to the server", error);
      report("offline");
    }
  })();
  return {
    show(next) {
      panel = next;
      panel.connection(status);
    },
  };
}

// The Scenes in order: Title into a solo race, the race into its results,
// and the results back into another race or the Title. The track comes
// from `?track=` until rooms pick it.
async function main(container: HTMLElement): Promise<void> {
  const track = selectTrack(window.location.search);
  const [bridge, sidecar] = await Promise.all([loadBridge(), fetchSidecar(track)]);
  const connection = reportConnection(bridge);

  const showTitle = () => {
    const title = createTitleScene(container, {
      race() {
        title.dispose();
        showRace().catch(fail);
      },
    });
  };
  const showRace = async () => {
    const race = await createRaceScene(container, bridge, track, sidecar);
    connection.show(race.panel);
    const results = await race.finished;
    race.dispose();
    const scene = createResultsScene(container, resultsRows(results), {
      rematch() {
        scene.dispose();
        showRace().catch(fail);
      },
      leave() {
        scene.dispose();
        showTitle();
      },
    });
  };
  showTitle();
}

function fail(error: unknown): void {
  console.error("Formula None failed", error);
}

const app = document.querySelector<HTMLDivElement>("#app");
if (app) {
  main(app).catch(fail);
}
