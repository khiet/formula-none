import type { KartTransform, Phase, Standing } from "./bridge";
import { type CourseSetup, createCourseMap } from "./course-map";
import { formatLapTime, NO_LAP_TIME, ordinal } from "./format";
import type { Item } from "./generated/protocol";

/**
 * The local kart's item slot as the run reports it: the item the server
 * says is held, and whether the fire is showing, predicted from the press
 * or confirmed by the server's event.
 */
export interface ItemSlot {
  readonly item: Item | null;
  readonly firing: boolean;
}

/** The item slot as the panel prints it: the item's name and the slot's state. */
export interface ItemReadout {
  readonly text: string;
  readonly state: "empty" | "held" | "firing";
}

export interface InstrumentPanel {
  /**
   * Show the local kart's readouts, the race's phase and the item slot from
   * what the run reports, and every kart on the course map at `transforms`,
   * the same ones the View draws this frame.
   */
  update(
    local: KartTransform,
    standing: Standing,
    phase: Phase,
    slot: ItemSlot,
    transforms: readonly KartTransform[],
  ): void;
  /** The connection readout: which transport reached the server and how fast, or that none did. */
  connection(status: string): void;
  /** Show the connection banner while a room's race has lost the server, and take it down once back. */
  connectionLost(lost: boolean): void;
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
export const CONNECTION_LOST_BANNER = "Connection lost: reconnecting";
/** How long GO stays up after the countdown. Presentation, so not the sim's to tune. */
const GO_S = 1;

/**
 * The HUD over the Race View, with the course map of `course` in its
 * corner. Reads state; never drives it. Each readout is addressable by
 * `data-instrument` so tests and styles find it the same way.
 */
export function createInstrumentPanel(
  container: HTMLElement,
  lapsPerRace: number,
  course: CourseSetup,
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
  const item = instrument("item");
  item.dataset.itemState = "empty";
  const banner = instrument("connection-banner", CONNECTION_LOST_BANNER);
  banner.hidden = true;
  const countdown = instrument("countdown");
  countdown.hidden = true;
  const speed = instrument("speed", formatSpeed(0));
  // The surface readout is a greybox aid: it shows the sidecar and the
  // mesh agree about where the kart is.
  const surface = instrument("surface");
  const connection = instrument("connection", "connecting");
  const courseMap = createCourseMap(course);
  panel.append(
    position,
    " ",
    lap,
    " ",
    lapTime,
    " ",
    bestLap,
    " ",
    item,
    " ",
    speed,
    " ",
    surface,
    " ",
    connection,
    " ",
    wrongWay,
    banner,
    countdown,
    courseMap.element,
  );
  container.append(panel);

  return {
    update(local, standing, phase, slot, transforms) {
      courseMap.update(transforms);
      const readouts = lapReadouts(standing, lapsPerRace);
      position.textContent = readouts.position;
      lap.textContent = readouts.lap;
      lapTime.textContent = readouts.lapTime;
      bestLap.textContent = readouts.bestLap;
      wrongWay.textContent = readouts.wrongWay;
      wrongWay.hidden = readouts.wrongWay === "";
      speed.textContent = formatSpeed(local.speed);
      surface.textContent = local.surface;
      const held = itemReadout(slot);
      item.textContent = held.text;
      item.dataset.itemState = held.state;
      const count = phaseReadout(phase);
      countdown.textContent = count;
      countdown.hidden = count === "";
    },
    connection(status) {
      connection.textContent = status;
    },
    connectionLost(lost) {
      banner.hidden = !lost;
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
    bestLap: `Best ${standing.bestLapSeconds === null ? NO_LAP_TIME : formatLapTime(standing.bestLapSeconds)}`,
    wrongWay: standing.wrongWay ? WRONG_WAY_WARNING : "",
  };
}

const ITEM_NAMES: Readonly<Record<Item, string>> = {
  boost: "Boost",
  hazard: "Hazard",
  homing: "Homing",
};

/** What the panel prints for the item slot: the item's name, in the firing state while the fire shows. */
export function itemReadout(slot: ItemSlot): ItemReadout {
  return {
    text: slot.item === null ? "" : ITEM_NAMES[slot.item],
    state: slot.firing ? "firing" : slot.item === null ? "empty" : "held",
  };
}

function formatSpeed(metresPerSecond: number): string {
  return `${metresPerSecond.toFixed(1)} m/s`;
}
