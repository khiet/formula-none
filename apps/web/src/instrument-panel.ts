import type { KartTransform } from "./bridge";

export interface InstrumentPanel {
  /** Show the local kart's readouts from what the sim reports. */
  update(local: KartTransform): void;
  /** The connection readout: which transport reached the server and how fast, or that none did. */
  connection(status: string): void;
}

/**
 * The HUD over the Race View. Reads state; never drives it. Each readout is
 * addressable by `data-instrument` so tests and styles find it the same way.
 */
export function createInstrumentPanel(container: HTMLElement): InstrumentPanel {
  const panel = document.createElement("div");
  panel.id = "instrument-panel";
  const speed = document.createElement("span");
  speed.dataset.instrument = "speed";
  speed.textContent = formatSpeed(0);
  // The surface readout is a greybox aid: it shows the sidecar and the
  // mesh agree about where the kart is.
  const surface = document.createElement("span");
  surface.dataset.instrument = "surface";
  const connection = document.createElement("span");
  connection.dataset.instrument = "connection";
  connection.textContent = "connecting";
  panel.append(speed, " ", surface, " ", connection);
  container.append(panel);

  return {
    update(local) {
      speed.textContent = formatSpeed(local.speed);
      surface.textContent = local.surface;
    },
    connection(status) {
      connection.textContent = status;
    },
  };
}

function formatSpeed(metresPerSecond: number): string {
  return `${metresPerSecond.toFixed(1)} m/s`;
}
