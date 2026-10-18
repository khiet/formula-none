import type { KartTransform } from "./bridge";

export interface InstrumentPanel {
  /** Show the local kart's speed, derived from how far it moved over the last tick. */
  update(before: KartTransform | undefined, after: KartTransform, tickSeconds: number): void;
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
    update(before, after, tickSeconds) {
      const metresPerSecond = before ? distance(before, after) / tickSeconds : 0;
      speed.textContent = formatSpeed(metresPerSecond);
    },
  };
}

function distance(a: KartTransform, b: KartTransform): number {
  const dx = b.position[0] - a.position[0];
  const dy = b.position[1] - a.position[1];
  const dz = b.position[2] - a.position[2];
  return Math.hypot(dx, dy, dz);
}

function formatSpeed(metresPerSecond: number): string {
  return `${metresPerSecond.toFixed(1)} m/s`;
}
