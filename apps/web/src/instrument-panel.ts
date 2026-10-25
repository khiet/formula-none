import instrumentFontUrl from "../../../assets/fonts/kenney/Kenney Mini Square.ttf?url";
import type { KartTransform, Phase, Standing } from "./bridge";
import { type CourseMapSetup, createCourseMap } from "./course-map";
import { formatLapTime, NO_LAP_TIME, ordinal, ordinalParts } from "./format";
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

/** A readout with its label word: the panel prints both, the label in white and the value in gold. */
export interface LabelledReadout {
  readonly label: string;
  readonly value: string;
}

/** The lap readouts as the panel prints them, one per instrument. */
export interface LapReadouts {
  readonly position: string;
  /** The lap count as text, which the panel keeps for assistive technology behind the flags. */
  readonly lap: string;
  /** How many of the race's laps are behind the kart: one checkered flag each. */
  readonly lapsCompleted: number;
  readonly lapTime: LabelledReadout;
  readonly bestLap: LabelledReadout;
  /** The wrong-way warning, or empty while the kart goes the right way. */
  readonly wrongWay: string;
}

const WRONG_WAY_WARNING = "WRONG WAY";
export const CONNECTION_LOST_BANNER = "Connection lost: reconnecting";
/** How long GO stays up after the countdown. Presentation, so not the sim's to tune. */
const GO_S = 1;

/** The family name the panel's styles ask for; the face is registered by `loadInstrumentFont`. */
export const INSTRUMENT_FONT_FAMILY = "Kenney Mini Square";

let instrumentFont: Promise<void> | undefined;

/**
 * Register the panel's pixel face (ADR 0023) with the document once, so the
 * Race Scene mounts with it ready and the first frame is set in it. A face
 * that fails to load leaves the panel in the fallback stack rather than
 * keeping the race from starting.
 */
export function loadInstrumentFont(): Promise<void> {
  instrumentFont ??= (async () => {
    if (typeof FontFace === "undefined") return;
    const face = new FontFace(INSTRUMENT_FONT_FAMILY, `url("${instrumentFontUrl}")`);
    document.fonts.add(face);
    try {
      await face.load();
    } catch (error) {
      console.warn("Instrument panel typeface did not load", error);
    }
  })();
  return instrumentFont;
}

/**
 * The HUD over the Race View, with the course map of `course` in its
 * corner. Reads state; never drives it. Each readout is addressable by
 * `data-instrument` so tests and styles find it the same way.
 */
export function createInstrumentPanel(
  container: HTMLElement,
  lapsPerRace: number,
  course: CourseMapSetup,
): InstrumentPanel {
  const panel = document.createElement("div");
  panel.id = "instrument-panel";
  const instrument = (name: string, text = ""): HTMLSpanElement => {
    const span = document.createElement("span");
    span.dataset.instrument = name;
    span.textContent = text;
    return span;
  };
  const child = (parent: HTMLElement, name: string): HTMLSpanElement => {
    const span = document.createElement("span");
    span.dataset[name] = "";
    parent.append(span);
    return span;
  };

  const position = instrument("position");
  const numeral = child(position, "numeral");
  const suffix = child(position, "suffix");

  // The lap count stays as text for assistive technology; the flags that
  // show it are decorative, one per lap of the race.
  const lap = instrument("lap");
  const lapText = child(lap, "lapText");
  const flagRow = child(lap, "flags");
  flagRow.setAttribute("aria-hidden", "true");
  const flags = Array.from({ length: lapsPerRace }, () => child(flagRow, "flag"));

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
  // The surface and connection readouts are greybox aids: the surface shows
  // the sidecar and the mesh agree about where the kart is. They sit in a
  // small line of their own, out of the driver's stack.
  const surface = instrument("surface");
  const connection = instrument("connection", "connecting");
  const greybox = document.createElement("span");
  greybox.dataset.greybox = "";
  greybox.append(surface, " ", connection);
  const stack = document.createElement("div");
  stack.dataset.stack = "";
  stack.append(position, lap, lapTime, bestLap, item, speed);
  const courseMap = createCourseMap(course);
  panel.append(stack, greybox, wrongWay, banner, countdown, courseMap.element);
  container.append(panel);

  const printLabelled = (element: HTMLElement, readout: LabelledReadout) => {
    const label = document.createElement("span");
    label.dataset.label = "";
    label.textContent = readout.label;
    element.replaceChildren(label, ` ${readout.value}`);
  };

  return {
    update(local, standing, phase, slot, transforms) {
      courseMap.update(transforms);
      const readouts = lapReadouts(standing, lapsPerRace);
      const parts = ordinalParts(standing.position);
      numeral.textContent = parts.numeral;
      suffix.textContent = parts.suffix;
      lapText.textContent = readouts.lap;
      flags.forEach((flag, index) => {
        flag.dataset.flag = index < readouts.lapsCompleted ? "done" : "todo";
      });
      printLabelled(lapTime, readouts.lapTime);
      printLabelled(bestLap, readouts.bestLap);
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

/**
 * What the panel prints for a standing: ordinal position, lap of the race's
 * laps with the laps behind the kart, and the two clocks with their labels.
 * A finished kart has every lap behind it whatever lap the sim leaves it on.
 */
export function lapReadouts(standing: Standing, lapsPerRace: number): LapReadouts {
  const lapsCompleted =
    standing.finishTimeSeconds === null
      ? Math.min(Math.max(standing.lap - 1, 0), lapsPerRace)
      : lapsPerRace;
  return {
    position: ordinal(standing.position),
    lap: `Lap ${standing.lap}/${lapsPerRace}`,
    lapsCompleted,
    lapTime: { label: "Time", value: formatLapTime(standing.lapTimeSeconds) },
    bestLap: {
      label: "Best",
      value:
        standing.bestLapSeconds === null ? NO_LAP_TIME : formatLapTime(standing.bestLapSeconds),
    },
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
