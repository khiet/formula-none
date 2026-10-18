import type { KartTransform, Standing } from "./bridge";

export interface InstrumentPanel {
  /** Show the local kart's readouts from what the sim reports. */
  update(local: KartTransform, standing: Standing): void;
  /** The connection readout: which transport reached the server and how fast, or that none did. */
  connection(status: string): void;
}

/** The lap readouts as the panel prints them, one string per instrument. */
export interface LapReadouts {
  readonly position: string;
  readonly lap: string;
  readonly lapTime: string;
  readonly bestLap: string;
}

/**
 * The HUD over the Race View. Reads state; never drives it. Each readout is
 * addressable by `data-instrument` so tests and styles find it the same way.
 */
export function createInstrumentPanel(
  container: HTMLElement,
  lapsPerRace: number,
): InstrumentPanel {
  const panel = document.createElement("div");
  panel.id = "instrument-panel";
  const instrument = (name: string, text = ""): HTMLSpanElement => {
    const span = document.createElement("span");
    span.dataset.instrument = name;
    span.textContent = text;
    return span;
  };
  const position = instrument("position");
  const lap = instrument("lap");
  const lapTime = instrument("lap-time");
  const bestLap = instrument("best-lap");
  const speed = instrument("speed", formatSpeed(0));
  // The surface readout is a greybox aid: it shows the sidecar and the
  // mesh agree about where the kart is.
  const surface = instrument("surface");
  const connection = instrument("connection", "connecting");
  panel.append(
    position,
    " ",
    lap,
    " ",
    lapTime,
    " ",
    bestLap,
    " ",
    speed,
    " ",
    surface,
    " ",
    connection,
  );
  container.append(panel);

  return {
    update(local, standing) {
      const readouts = lapReadouts(standing, lapsPerRace);
      position.textContent = readouts.position;
      lap.textContent = readouts.lap;
      lapTime.textContent = readouts.lapTime;
      bestLap.textContent = readouts.bestLap;
      speed.textContent = formatSpeed(local.speed);
      surface.textContent = local.surface;
    },
    connection(status) {
      connection.textContent = status;
    },
  };
}

/** What the panel prints for a standing: ordinal position, lap of the race's laps, and the two clocks. */
export function lapReadouts(standing: Standing, lapsPerRace: number): LapReadouts {
  return {
    position: ordinal(standing.position),
    lap: `Lap ${standing.lap}/${lapsPerRace}`,
    lapTime: formatLapTime(standing.lapTimeSeconds),
    bestLap: `Best ${standing.bestLapSeconds === null ? "--:--.---" : formatLapTime(standing.bestLapSeconds)}`,
  };
}

/** 1st, 2nd, 3rd, 4th and so on, for the eight positions a race can have. */
function ordinal(position: number): string {
  const suffix = { 1: "st", 2: "nd", 3: "rd" }[position] ?? "th";
  return `${position}${suffix}`;
}

/** Minutes, seconds and milliseconds: 1:15.457. */
function formatLapTime(seconds: number): string {
  const wholeMilliseconds = Math.round(seconds * 1000);
  const minutes = Math.floor(wholeMilliseconds / 60_000);
  const rest = wholeMilliseconds - minutes * 60_000;
  const wholeSeconds = Math.floor(rest / 1000);
  const milliseconds = rest - wholeSeconds * 1000;
  return `${minutes}:${String(wholeSeconds).padStart(2, "0")}.${String(milliseconds).padStart(3, "0")}`;
}

function formatSpeed(metresPerSecond: number): string {
  return `${metresPerSecond.toFixed(1)} m/s`;
}
