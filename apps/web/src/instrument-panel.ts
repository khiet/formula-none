import type { KartTransform } from "./bridge";

export interface InstrumentPanel {
  /** Show the local kart's readouts from what the sim reports. */
  update(local: KartTransform): void;
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
  panel.append(speed);
  container.append(panel);

  return {
    update(local) {
      speed.textContent = formatSpeed(local.speed);
    },
  };
}

function formatSpeed(metresPerSecond: number): string {
  return `${metresPerSecond.toFixed(1)} m/s`;
}
