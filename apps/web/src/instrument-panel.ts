import instrumentFontUrl from "../../../assets/fonts/kenney/Kenney Mini Square.ttf?url";
import type { Phase, Standing } from "./bridge";
import { type CourseMapSetup, createCourseMap } from "./course-map";
import {
  formatLapTime,
  NO_LAP_TIME,
  type OrdinalParts,
  ordinal,
  ordinalParts,
  speedParts,
} from "./format";
import type { Item } from "./generated/protocol";
import { itemSprite } from "./item-sprites";
import { hexCss, type Livery, livery as liveryOf } from "./livery";
import {
  facetted,
  group,
  line,
  ngon,
  type Point,
  polygon,
  rectangle,
  SVG_NS,
  svg,
} from "./low-poly-svg";
import {
  CARBON,
  FIRING,
  GO_GREEN,
  HAZARD_ORANGE,
  INK,
  ITEM_BOX_GOLD,
  LABEL_GREY,
  POSTER_RED,
  plate,
  tonesOf,
  VALUE_WHITE,
  WHITE,
  writePalette,
  writeTones,
} from "./palette";
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

/** A readout with its label word: the panel prints both, the label in grey and the value in white. */
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

/** One finisher in the standings a finished kart sees. */
export interface FinisherRow {
  readonly position: string;
  readonly name: string;
  readonly time: string;
}

/** What the panel shows in place of the lap once the own kart has finished. */
export interface FinishReadout {
  readonly banner: string;
  /** The own kart's place, as an ordinal. */
  readonly place: string;
  /** Every kart finished so far, in finishing order. */
  readonly rows: readonly FinisherRow[];
}

const WRONG_WAY_WARNING = "WRONG WAY";
const FINISH_BANNER = "FINISH";
export const CONNECTION_LOST_BANNER = "Connection lost: reconnecting";

/** The family name the panel's styles ask for; the face is registered by `loadInstrumentFont`. */
const INSTRUMENT_FONT_FAMILY = "Kenney Mini Square";

let instrumentFont: Promise<void> | undefined;

/**
 * Register the panel's pixel face (ADR 0023) with the document once, so the
 * Scenes that use it mount with it ready and their first frame is set in it.
 * A face that fails to load leaves them in the fallback stack rather than
 * keeping the game from starting.
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
 * 0031); once the own kart has finished or the race has ended, the
 * finish banner and the finishers by `name` in place of the lap. Reads state; never drives it.
 * The own kart's readouts wear `paint`, the own kart's livery (ADR 0037).
 * Each readout is addressable by `data-instrument` so tests and styles
 * find it the same way.
 */
export function createInstrumentPanel(
  container: HTMLElement,
  lapsPerRace: number,
  course: CourseMapSetup,
  topSpeed: number,
  name: (kart: number) => string,
  paint: Livery,
): InstrumentPanel {
  const panel = document.createElement("div");
  panel.id = "instrument-panel";
  writePalette(panel);
  writeTones(panel, "primary", tonesOf(paint.primary));
  writeTones(panel, "secondary", tonesOf(paint.secondary));
  writeTones(panel, "accent", tonesOf(paint.accent));
  panel.style.setProperty("--key", hexCss(paint.key));
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
  plate(position, tonesOf(paint.primary));
  const numeral = part(position, "numeral");
  const suffix = part(position, "suffix");

  // The lap count stays as text for assistive technology; the flags that
  // show it are decorative, one per lap of the race.
  const lap = instrument("lap");
  plate(lap, CARBON);
  const lapText = part(lap, "lapText");
  const flagRow = part(lap, "flags");
  flagRow.setAttribute("aria-hidden", "true");
  const flags = Array.from({ length: lapsPerRace }, () => {
    const flag = lapFlag(paint);
    flagRow.append(flag);
    return flag;
  });

  const lapTime = instrument("lap-time");
  plate(lapTime, CARBON);
  const bestLap = instrument("best-lap");
  plate(bestLap, CARBON);
  const wrongWay = instrument("wrong-way");
  plate(wrongWay, HAZARD_ORANGE);
  wrongWay.hidden = true;
  // The slot's text is the item's name, kept for assistive technology and
  // visually hidden behind the sprite (ADR 0024).
  const item = instrument("item");
  plate(item, ITEM_BOX_GOLD);
  item.dataset.itemState = "empty";
  const itemName = part(item, "itemName");
  let shownItem: Item | null = null;
  let shownSprite: SVGSVGElement | null = null;
  const banner = instrument("connection-banner", CONNECTION_LOST_BANNER);
  plate(banner, POSTER_RED);
  banner.hidden = true;
  // The start lights: five octagonal lamps, lit one a second; GO on a
  // plate of its own once they go out.
  const countdown = instrument("countdown");
  countdown.hidden = true;
  const lamps = Array.from({ length: START_LIGHTS }, () => {
    const lamp = part(countdown, "lamp");
    lamp.dataset.plate = "octagon";
    lamp.dataset.lamp = "off";
    return lamp;
  });
  const go = part(countdown, "go");
  const finish = instrument("finish");
  plate(finish, CARBON);
  finish.hidden = true;
  const standingsList = document.createElement("ol");
  standingsList.dataset.instrument = "standings";
  standingsList.hidden = true;
  // The speed cluster (ADR 0031): the gear, speed and tyres stay as text;
  // the shift lights and the kart from above are decorative.
  const cluster = document.createElement("div");
  cluster.dataset.cluster = "";
  const lightRow = svg(`0 0 ${SHIFT_LIGHT_PITCH * 12} 14`);
  lightRow.dataset.shiftLights = "";
  cluster.append(lightRow);
  const lights = shiftLights(drivetrain(0, topSpeed)).map((state, index) => {
    const light = group(lightRow);
    light.dataset.shiftLight = state;
    facetted(light, 6, SHIFT_LIGHT_PITCH / 2 + index * SHIFT_LIGHT_PITCH, 7, 6.2);
    return light;
  });
  const gear = instrument("gear");
  const speed = instrument("speed");
  const speedValue = part(speed, "speedValue");
  speed.append(" ");
  const speedUnit = part(speed, "speedUnit");
  const tyre = instrument("tyre");
  tyre.append(kartFromAbove(paint));
  const tyrePercent = part(tyre, "tyrePercent");
  cluster.append(gear, speed, tyre);
  const showSpeed = (metresPerSecond: number) => {
    const reading = drivetrain(metresPerSecond, topSpeed);
    shiftLights(reading).forEach((state, index) => {
      const light = lights[index];
      if (light) light.dataset.shiftLight = state;
    });
    gear.textContent = reading.gear;
    if (gear.dataset.limiter !== String(reading.limiter)) {
      gear.dataset.limiter = String(reading.limiter);
      plate(gear, reading.limiter ? WHITE : CARBON);
    }
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
  panel.append(
    stack,
    cluster,
    greybox,
    wrongWay,
    banner,
    countdown,
    finish,
    standingsList,
    courseMap.element,
  );
  container.append(panel);

  const standingRows = (rows: readonly FinisherRow[], karts: readonly number[]) =>
    rows.map((row, index) => {
      const kart = karts[index] ?? 0;
      const own = kart === course.ownKart;
      const element = document.createElement("li");
      plate(element, own ? ITEM_BOX_GOLD : CARBON);
      if (own) element.dataset.own = "";
      const place = document.createElement("b");
      place.textContent = row.position;
      element.append(place, " ", paletteChip(liveryOf(kart)), `${row.name} ${row.time}`);
      return element;
    });
  let shownLit = 0;
  let shownCount = "";
  let shownItemState = "";

  const printLabelled = (element: HTMLElement, readout: LabelledReadout) => {
    const label = document.createElement("span");
    label.dataset.label = "";
    label.textContent = readout.label;
    // Kenney Mini Square has no tabular figures and its 1 is narrower than
    // the other digits, so each figure gets a fixed cell from the stylesheet.
    // The figures share one span so the plate's flex gap falls only between
    // the label and the value.
    const value = document.createElement("span");
    value.append(
      ...[...readout.value].map((character) => {
        if (!/[0-9-]/.test(character)) return character;
        const figure = document.createElement("span");
        figure.dataset.figure = "";
        figure.textContent = character;
        return figure;
      }),
    );
    element.replaceChildren(label, " ", value);
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
      if (held.state !== shownItemState) {
        shownItemState = held.state;
        item.dataset.itemState = held.state;
        plate(item, held.state === "firing" ? FIRING : ITEM_BOX_GOLD);
      }
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
      const lit = lampsLit(race.phase);
      const count = phaseReadout(race.phase);
      if (lit !== shownLit || count !== shownCount) {
        shownLit = lit;
        shownCount = count;
        countdown.dataset.lit = String(lit);
        countdown.hidden = lit === 0 && count === "";
        lamps.forEach((lamp, index) => {
          lamp.dataset.lamp = index < lit ? "lit" : "off";
          lamp.hidden = count !== "";
        });
        go.textContent = count;
        // An empty GO would still take a gap in the row and push the lamps off centre.
        go.hidden = count === "";
        if (count === "") {
          delete countdown.dataset.plate;
        } else {
          plate(countdown, GO_GREEN);
        }
      }
      const finished = finishReadout(race.standings, race.phase, course.ownKart, name);
      lap.hidden = finished !== null;
      finish.hidden = finished === null;
      standingsList.hidden = finished === null;
      if (finished !== null) {
        finish.replaceChildren(chequer(), `${finished.banner} ${finished.place}`, chequer());
        // Rebuilt only as karts finish, so a frame changes nothing.
        if (standingsList.children.length !== finished.rows.length) {
          standingsList.replaceChildren(
            ...standingRows(finished.rows, finishingOrder(race.standings)),
          );
        }
      }
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

/** The start lights the panel draws; the countdown's tuning must give one a second. */
export const START_LIGHTS = 5;

/** The start lights lit at `phase`: one more each second of the countdown, none from Go. */
export function lampsLit(phase: Phase): number {
  const count = raceCount(phase);
  return typeof count === "number" ? START_LIGHTS - count + 1 : 0;
}

/** What the panel prints over the View: GO at lights out, nothing over the lamps or after. */
export function phaseReadout(phase: Phase): string {
  return raceCount(phase) === "go" ? "GO" : "";
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

/**
 * The finish as the own kart in `ownKart` sees it from `standings`, with
 * each kart named by `name`, once it has finished or `phase` is the outro;
 * nothing while it is still racing. A finisher's place is final, so the
 * rows only ever grow.
 */
export function finishReadout(
  standings: readonly Standing[],
  phase: Phase,
  ownKart: number,
  name: (kart: number) => string,
): FinishReadout | null {
  const own = standings[ownKart];
  const over = phase.kind === "outro" || phase.kind === "finished";
  if (own === undefined || (own.finishTimeS === null && !over)) return null;
  const rows = finishingOrder(standings).map((kart) => {
    const standing = standings[kart];
    return {
      position: ordinal(standing?.position ?? 0),
      name: name(kart),
      time: formatLapTime(standing?.finishTimeS ?? 0),
    };
  });
  return { banner: FINISH_BANNER, place: ordinal(own.position), rows };
}

/** The karts finished so far in `standings`, in finishing order. */
function finishingOrder(standings: readonly Standing[]): number[] {
  return standings
    .map((standing, kart) => ({ standing, kart }))
    .filter(({ standing }) => standing.finishTimeS !== null)
    .sort((a, b) => a.standing.position - b.standing.position)
    .map(({ kart }) => kart);
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

/** The shift lights' spacing in their drawing's units: each hexagon 12.4 across with a gap. */
const SHIFT_LIGHT_PITCH = 13;

let chequerPatterns = 0;

/**
 * A lap flag folded once, lighter on the left half, in the own kart's accent;
 * chequered once its lap is done. Both faces are drawn and the flag's
 * `data-flag` picks which shows, so a lap changes one attribute.
 */
function lapFlag(paint: Livery): SVGSVGElement {
  const flag = svg("0 0 14 14");
  flag.dataset.flag = "todo";
  const left: Point[] = [
    [2.4, 1],
    [8, 2.2],
    [8, 8.4],
    [2.4, 7.2],
  ];
  const right: Point[] = [
    [8, 2.2],
    [13.4, 1],
    [13.4, 7.2],
    [8, 8.4],
  ];
  polygon(
    flag,
    [
      [1, 1],
      [2.4, 1],
      [2.4, 13.5],
      [1, 13.5],
    ],
    LABEL_GREY,
  );
  const accent = tonesOf(paint.accent);
  const todo = group(flag);
  todo.dataset.fold = "todo";
  polygon(todo, left, accent.mid);
  polygon(todo, right, accent.shaded);

  const id = `lap-flag-chequer-${chequerPatterns++}`;
  const pattern = document.createElementNS(SVG_NS, "pattern");
  pattern.id = id;
  for (const [name, value] of Object.entries({ width: 3, height: 3 })) {
    pattern.setAttribute(name, String(value));
  }
  pattern.setAttribute("patternUnits", "userSpaceOnUse");
  polygon(pattern, rectangle(0, 0, 3, 3), VALUE_WHITE);
  polygon(pattern, rectangle(0, 0, 1.5, 1.5), INK);
  polygon(pattern, rectangle(1.5, 1.5, 3, 3), INK);
  const done = group(flag);
  done.dataset.fold = "done";
  done.append(pattern);
  polygon(done, left, `url(#${id})`);
  polygon(done, right, `url(#${id})`);
  polygon(done, right, INK).setAttribute("fill-opacity", "0.28");
  return flag;
}

/** One of FINISH's chequered ends, decorative. */
function chequer(): HTMLElement {
  const end = document.createElement("i");
  end.dataset.chequer = "";
  end.setAttribute("aria-hidden", "true");
  return end;
}

/** A kart's primary, secondary and accent side by side, as on the Results board. */
function paletteChip(paint: Livery): HTMLElement {
  const chip = document.createElement("span");
  chip.dataset.chip = "";
  chip.setAttribute("aria-hidden", "true");
  for (const colour of [paint.primary, paint.secondary, paint.accent]) {
    const stripe = document.createElement("i");
    stripe.style.background = hexCss(colour);
    chip.append(stripe);
  }
  return chip;
}

/**
 * A tyre seen from above at `x`, `y`, `width` by `height`: a chamfered
 * block lit on the left, marked `data-tyre-corner` so the panel's styles
 * tone it by the tyre gauge's band.
 */
function tyreFromAbove(parent: Element, x: number, y: number, width: number, height: number): void {
  const tyre = group(parent);
  tyre.dataset.tyreCorner = "";
  const c = 2;
  const block: Point[] = [
    [x + c, y],
    [x + width - c, y],
    [x + width, y + c],
    [x + width, y + height - c],
    [x + width - c, y + height],
    [x + c, y + height],
    [x, y + height - c],
    [x, y + c],
  ];
  const litEdge = x + width * 0.34;
  const shadedEdge = x + width * 0.7;
  const faces: [Point[], number][] = [
    [block, 1],
    [
      [
        [x + c, y],
        [litEdge, y],
        [litEdge, y + height],
        [x + c, y + height],
        [x, y + height - c],
        [x, y + c],
      ],
      0,
    ],
    [
      [
        [shadedEdge, y],
        [x + width - c, y],
        [x + width, y + c],
        [x + width, y + height - c],
        [x + width - c, y + height],
        [shadedEdge, y + height],
      ],
      2,
    ],
  ];
  for (const [points, tone] of faces) {
    polygon(tyre, points).dataset.tone = String(tone);
  }
}

/**
 * The own kart from above for the tyre gauge (ADR 0037): wings in
 * secondary, tub and sidepods in primary lit on the left, the accent
 * stripe, the halo and the driver's hair in the cockpit, the rears bigger
 * than the fronts. Decorative: the gauge's percentage is the text.
 */
function kartFromAbove(paint: Livery): SVGSVGElement {
  const kart = svg("0 0 60 92");
  const primary = tonesOf(paint.primary);
  const secondary = tonesOf(paint.secondary);
  const accent = paint.accent;
  for (const [x1, x2, y] of [
    [22, 12, 22],
    [38, 48, 22],
    [20, 12, 70],
    [40, 48, 70],
  ] as const) {
    line(kart, [x1, y], [x2, y], CARBON.shaded, 1.5);
  }
  tyreFromAbove(kart, 2, 14, 11, 17);
  tyreFromAbove(kart, 47, 14, 11, 17);
  tyreFromAbove(kart, 0, 58, 13, 24);
  tyreFromAbove(kart, 47, 58, 13, 24);
  polygon(
    kart,
    [
      [18, 40],
      [42, 40],
      [45, 64],
      [38, 76],
      [22, 76],
      [15, 64],
    ],
    primary.mid,
  );
  polygon(
    kart,
    [
      [18, 40],
      [30, 40],
      [30, 76],
      [22, 76],
      [15, 64],
    ],
    primary.lit,
  );
  polygon(
    kart,
    [
      [38, 52],
      [42, 40],
      [45, 64],
      [38, 76],
    ],
    primary.shaded,
  );
  polygon(
    kart,
    [
      [26, 6],
      [34, 6],
      [37, 42],
      [23, 42],
    ],
    primary.mid,
  );
  polygon(
    kart,
    [
      [26, 6],
      [30, 6],
      [30, 42],
      [23, 42],
    ],
    primary.lit,
  );
  polygon(kart, rectangle(29, 8, 31, 78), accent);
  polygon(kart, rectangle(6, 4, 54, 11), secondary.mid);
  polygon(kart, rectangle(6, 4, 54, 6.5), secondary.lit);
  polygon(kart, rectangle(4, 3, 7, 12), accent);
  polygon(kart, rectangle(53, 3, 56, 12), accent);
  polygon(kart, ngon(6, 30, 51, 8), CARBON.shaded);
  facetted(kart, 8, 30, 52, 5.6, tonesOf(paint.driver.hairColour));
  const halo = document.createElementNS(SVG_NS, "polyline");
  halo.setAttribute("points", "22.5,55 23.5,45 30,41.5 36.5,45 37.5,55");
  halo.setAttribute("stroke", hexCss(INK));
  halo.setAttribute("stroke-width", "2");
  halo.setAttribute("stroke-linejoin", "round");
  halo.setAttribute("fill", "none");
  kart.append(halo);
  polygon(
    kart,
    [
      [24, 66],
      [36, 66],
      [34, 82],
      [26, 82],
    ],
    primary.shaded,
  );
  polygon(kart, rectangle(8, 80, 52, 89), secondary.mid);
  polygon(kart, rectangle(8, 80, 52, 83), secondary.lit);
  polygon(kart, rectangle(6, 79, 9, 90), accent);
  polygon(kart, rectangle(51, 79, 54, 90), accent);
  return kart;
}
