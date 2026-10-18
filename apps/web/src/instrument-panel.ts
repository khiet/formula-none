import type { KartTransform, Phase, Standing } from "./bridge";

export interface InstrumentPanel {
  /** Show the local kart's readouts and the race's phase from what the sim reports. */
  update(local: KartTransform, standing: Standing, phase: Phase): void;
  /** The connection readout: which transport reached the server and how fast, or that none did. */
  connection(status: string): void;
  /** Take the panel off the page. */
  dispose(): void;
}

/** The lap readouts as the panel prints them, one string per instrument. */
export interface LapReadouts {
  readonly position: string;
  readonly lap: string;
  readonly lapTime: string;
  readonly bestLap: string;
  /** The wrong-way warning, or empty while the kart goes the right way. */
  readonly wrongWay: string;
}

const WRONG_WAY_WARNING = "WRONG WAY";
/** How long GO stays up after the countdown. Presentation, so not the sim's to tune. */
const GO_S = 1;

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
  const wrongWay = instrument("wrong-way");
  wrongWay.hidden = true;
  const countdown = instrument("countdown");
  countdown.hidden = true;
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
    " ",
    wrongWay,
    countdown,
  );
  container.append(panel);

  return {
    update(local, standing, phase) {
      const readouts = lapReadouts(standing, lapsPerRace);
      position.textContent = readouts.position;
      lap.textContent = readouts.lap;
      lapTime.textContent = readouts.lapTime;
      bestLap.textContent = readouts.bestLap;
      wrongWay.textContent = readouts.wrongWay;
      wrongWay.hidden = readouts.wrongWay === "";
      speed.textContent = formatSpeed(local.speed);
      surface.textContent = local.surface;
      const count = phaseReadout(phase);
      countdown.textContent = count;
      countdown.hidden = count === "";
    },
    connection(status) {
      connection.textContent = status;
    },
    dispose() {
      panel.remove();
    },
  };
}

/**
 * The count over the View: the whole seconds left to Go, GO for the first
 * second of the race, and nothing the rest of the time.
 */
export function phaseReadout(phase: Phase): string {
  switch (phase.kind) {
    case "countdown":
      return String(Math.ceil(phase.remainingSeconds));
    case "racing":
      return phase.elapsedSeconds < GO_S ? "GO" : "";
    case "finished":
      return "";
  }
}

/** What the panel prints for a standing: ordinal position, lap of the race's laps, and the two clocks. */
export function lapReadouts(standing: Standing, lapsPerRace: number): LapReadouts {
  return {
    position: ordinal(standing.position),
    lap: `Lap ${standing.lap}/${lapsPerRace}`,
    lapTime: formatLapTime(standing.lapTimeSeconds),
    bestLap: `Best ${standing.bestLapSeconds === null ? "--:--.---" : formatLapTime(standing.bestLapSeconds)}`,
    wrongWay: standing.wrongWay ? WRONG_WAY_WARNING : "",
  };
}

/** 1st, 2nd, 3rd, 4th and so on, for the eight positions a race can have. */
export function ordinal(position: number): string {
  const suffix = { 1: "st", 2: "nd", 3: "rd" }[position] ?? "th";
  return `${position}${suffix}`;
}

/** Minutes, seconds and milliseconds: 1:15.457. */
export function formatLapTime(seconds: number): string {
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
