import instrumentFontUrl from "../../../assets/fonts/kenney/Kenney Mini Square.ttf?url";
import type { Phase, Standing } from "./bridge";
import { type CourseMapSetup, createCourseMap } from "./course-map";
import { formatLapTime, NO_LAP_TIME, type OrdinalParts, ordinalParts, speedParts } from "./format";
import type { Item } from "./generated/protocol";
import { itemSprite } from "./item-sprites";
import { type ItemSlot, type RaceReadout, raceCount } from "./race-run";
import {
  drivetrain,
  shiftLights,
  TYRE_WEAR_PLACEHOLDER_PERCENT,
  tyreWearBand,
} from "./speed-cluster";

/** The item slot as the panel prints it: the item's name and the slot's state. */
export interface ItemReadout {
  readonly text: string;
  readonly state: "empty" | "held" | "firing";
}

export interface InstrumentPanel {
  /**
   * Show the own kart's readouts, the race's phase and the item slot from
   * `race`, and every kart on the course map where the View draws it this
   * frame. Shows nothing new while `race` has no own kart.
   */
  update(race: RaceReadout): void;
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
  /** The position as a large numeral and a small suffix. */
  readonly position: OrdinalParts;
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

/** The family name the panel's styles ask for; the face is registered by `loadInstrumentFont`. */
const INSTRUMENT_FONT_FAMILY = "Kenney Mini Square";

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
 * corner and the speed cluster's gears cut from `topSpeed` in m/s (ADR
 * 0031). Reads state; never drives it. Each readout is addressable by
 * `data-instrument` so tests and styles find it the same way.
 */
export function createInstrumentPanel(
  container: HTMLElement,
  lapsPerRace: number,
  course: CourseMapSetup,
  topSpeed: number,
): InstrumentPanel {
  const panel = document.createElement("div");
  panel.id = "instrument-panel";
  const instrument = (name: string, text = ""): HTMLSpanElement => {
    const span = document.createElement("span");
    span.dataset.instrument = name;
    span.textContent = text;
    return span;
  };
  // A part of an instrument, marked by a data attribute so styles and tests find it.
  const part = (parent: HTMLElement, name: string): HTMLSpanElement => {
    const span = document.createElement("span");
    span.dataset[name] = "";
    parent.append(span);
    return span;
  };

  const position = instrument("position");
  const numeral = part(position, "numeral");
  const suffix = part(position, "suffix");

  // The lap count stays as text for assistive technology; the flags that
  // show it are decorative, one per lap of the race.
  const lap = instrument("lap");
  const lapText = part(lap, "lapText");
  const flagRow = part(lap, "flags");
  flagRow.setAttribute("aria-hidden", "true");
  const flags = Array.from({ length: lapsPerRace }, () => part(flagRow, "flag"));

  const lapTime = instrument("lap-time");
  const bestLap = instrument("best-lap");
  const wrongWay = instrument("wrong-way");
  wrongWay.hidden = true;
  // The slot's text is the item's name, kept for assistive technology and
  // visually hidden behind the sprite (ADR 0024).
  const item = instrument("item");
  item.dataset.itemState = "empty";
  const itemName = part(item, "itemName");
  let shownItem: Item | null = null;
  let shownSprite: SVGSVGElement | null = null;
  const banner = instrument("connection-banner", CONNECTION_LOST_BANNER);
  banner.hidden = true;
  const countdown = instrument("countdown");
  countdown.hidden = true;
  // The speed cluster (ADR 0031): the gear, speed and tyres stay as text;
  // the shift lights and the kart icon are decorative.
  const cluster = document.createElement("div");
  cluster.dataset.cluster = "";
  const lightRow = part(cluster, "shiftLights");
  lightRow.setAttribute("aria-hidden", "true");
  const lights = shiftLights(drivetrain(0, topSpeed)).map((state) => {
    const light = part(lightRow, "shiftLight");
    light.dataset.shiftLight = state;
    return light;
  });
  const gear = instrument("gear");
  const speed = instrument("speed");
  const speedValue = part(speed, "speedValue");
  speed.append(" ");
  const speedUnit = part(speed, "speedUnit");
  const tyre = instrument("tyre");
  tyre.append(kartIcon());
  const tyrePercent = part(tyre, "tyrePercent");
  cluster.append(gear, speed, tyre);
  const showSpeed = (metresPerSecond: number) => {
    const reading = drivetrain(metresPerSecond, topSpeed);
    shiftLights(reading).forEach((state, index) => {
      const light = lights[index];
      if (light) light.dataset.shiftLight = state;
    });
    gear.textContent = reading.gear;
    gear.dataset.limiter = String(reading.limiter);
    const parts = speedParts(metresPerSecond);
    speedValue.textContent = parts.value;
    speedUnit.textContent = parts.unit;
  };
  showSpeed(0);
  tyrePercent.textContent = `${TYRE_WEAR_PLACEHOLDER_PERCENT}%`;
  tyre.dataset.tyreWear = tyreWearBand(TYRE_WEAR_PLACEHOLDER_PERCENT);
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
  stack.append(position, lap, lapTime, bestLap, item);
  const courseMap = createCourseMap(course);
  panel.append(stack, cluster, greybox, wrongWay, banner, countdown, courseMap.element);
  container.append(panel);

  const printLabelled = (element: HTMLElement, readout: LabelledReadout) => {
    const label = document.createElement("span");
    label.dataset.label = "";
    label.textContent = readout.label;
    element.replaceChildren(label, ` ${readout.value}`);
  };

  return {
    update(race) {
      const local = race.karts[course.ownKart];
      const standing = race.standings[course.ownKart];
      if (local === undefined || standing === undefined) return;
      const slot = race.itemSlot;
      courseMap.update(race.karts);
      const readouts = lapReadouts(standing, lapsPerRace);
      numeral.textContent = readouts.position.numeral;
      suffix.textContent = readouts.position.suffix;
      lapText.textContent = readouts.lap;
      flags.forEach((flag, index) => {
        flag.dataset.flag = index < readouts.lapsCompleted ? "done" : "todo";
      });
      printLabelled(lapTime, readouts.lapTime);
      printLabelled(bestLap, readouts.bestLap);
      wrongWay.textContent = readouts.wrongWay;
      wrongWay.hidden = readouts.wrongWay === "";
      showSpeed(local.speed);
      surface.textContent = local.surface;
      const held = itemReadout(slot);
      item.dataset.itemState = held.state;
      if (slot.item !== shownItem) {
        shownItem = slot.item;
        itemName.textContent = held.text;
        shownSprite?.remove();
        shownSprite = null;
        if (slot.item === null) {
          delete item.dataset.item;
        } else {
          item.dataset.item = slot.item;
          shownSprite = itemSprite(slot.item);
          item.append(shownSprite);
        }
      }
      const count = phaseReadout(race.phase);
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

/** The race's count as the panel prints it over the View: 3, 2, 1, GO, or nothing. */
export function phaseReadout(phase: Phase): string {
  const count = raceCount(phase);
  return count === null ? "" : count === "go" ? "GO" : String(count);
}

/**
 * What the panel prints for a standing: ordinal position, lap of the race's
 * laps with the laps behind the kart, and the two clocks with their labels.
 * A finished kart has every lap behind it whatever lap the sim leaves it on.
 */
export function lapReadouts(standing: Standing, lapsPerRace: number): LapReadouts {
  const lapsCompleted =
    standing.finishTimeS === null
      ? Math.min(Math.max(standing.lap - 1, 0), lapsPerRace)
      : lapsPerRace;
  return {
    position: ordinalParts(standing.position),
    lap: `Lap ${standing.lap}/${lapsPerRace}`,
    lapsCompleted,
    lapTime: { label: "Time", value: formatLapTime(standing.lapTimeS) },
    bestLap: {
      label: "Best",
      value: standing.bestLapS === null ? NO_LAP_TIME : formatLapTime(standing.bestLapS),
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

const SVG_NS = "http://www.w3.org/2000/svg";

/**
 * A pixel kart from above with its four tyres marked `data-tyre-corner`, so
 * the panel's styles colour them by the tyre gauge's band. Decorative: the
 * gauge's percentage is the text.
 */
function kartIcon(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 14 18");
  svg.setAttribute("shape-rendering", "crispEdges");
  svg.setAttribute("aria-hidden", "true");
  const rect = (x: number, y: number, width: number, height: number, fill?: string) => {
    const shape = document.createElementNS(SVG_NS, "rect");
    for (const [name, value] of Object.entries({ x, y, width, height })) {
      shape.setAttribute(name, String(value));
    }
    if (fill !== undefined) shape.setAttribute("fill", fill);
    svg.append(shape);
    return shape;
  };
  rect(4, 2, 6, 14, "#3a1c0a");
  rect(5, 3, 4, 12, "#d62828");
  rect(6, 5, 2, 3, "#7fc3ff");
  for (const [x, y] of [
    [0, 2],
    [11, 2],
    [0, 11],
    [11, 11],
  ] as const) {
    rect(x, y, 3, 5).dataset.tyreCorner = "";
  }
  return svg;
}
