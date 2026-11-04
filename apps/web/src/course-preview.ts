// A course's card: its name and the circuit it is after, the flat map with
// a dot lapping it, a blurb and four numbers measured from the sidecar's
// centreline, so they can never disagree with the track the sim races.

import { projectCentreline } from "./course-map";
import { hexCss } from "./livery";
import { SVG_NS } from "./low-poly-svg";
import { ASPHALT, COIN_GOLD, INK, VALUE_WHITE, WALL } from "./palette";
import type { Course, WorldPoint } from "./track";

export interface CourseStats {
  /** Once round the centreline, in metres. */
  readonly lapM: number;
  /** The radius of the tightest bend, in metres. */
  readonly tightestM: number;
  /** The longest run with no bend in it, in metres. */
  readonly straightM: number;
}

/** How many points either side a bend's radius is measured across: wide enough to ride over the sidecar's rounding. */
const RADIUS_SPAN = 3;
/** The turn, in radians, under which a point still counts as on a straight. */
const STRAIGHT_TURN = 0.005;

/** The radius of the circle through `a`, `b` and `c` on the x/z plane; infinite when they are in a line. */
function circumradius(a: WorldPoint, b: WorldPoint, c: WorldPoint): number {
  const ab = Math.hypot(b[0] - a[0], b[2] - a[2]);
  const bc = Math.hypot(c[0] - b[0], c[2] - b[2]);
  const ca = Math.hypot(a[0] - c[0], a[2] - c[2]);
  const cross = Math.abs((b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0]));
  return cross < 1e-9 ? Number.POSITIVE_INFINITY : (ab * bc * ca) / (2 * cross);
}

/** How far the course turns at `b`, coming from `a` and going on to `c`, in radians. */
function turnAt(a: WorldPoint, b: WorldPoint, c: WorldPoint): number {
  const before = Math.atan2(b[2] - a[2], b[0] - a[0]);
  const after = Math.atan2(c[2] - b[2], c[0] - b[0]);
  const turn = (after - before) % (2 * Math.PI);
  return Math.abs(
    turn > Math.PI ? turn - 2 * Math.PI : turn < -Math.PI ? turn + 2 * Math.PI : turn,
  );
}

/**
 * The lap, the tightest bend and the longest straight of the closed
 * course through `centreline`, which the exporter samples about a metre
 * apart along arcs and straights.
 */
export function courseStats(centreline: readonly WorldPoint[]): CourseStats {
  const count = centreline.length;
  const at = (i: number) => centreline[((i % count) + count) % count] as WorldPoint;
  const step = (i: number) => {
    const [a, b] = [at(i), at(i + 1)];
    return Math.hypot(b[0] - a[0], b[2] - a[2]);
  };
  let lapM = 0;
  let tightestM = Number.POSITIVE_INFINITY;
  for (let i = 0; i < count; i += 1) {
    lapM += step(i);
    tightestM = Math.min(tightestM, circumradius(at(i - RADIUS_SPAN), at(i), at(i + RADIUS_SPAN)));
  }
  // A step is on a straight when the course does not turn at either end of
  // it; the lap is walked twice so a straight through the start is whole.
  const straightAt = (i: number) => turnAt(at(i - 1), at(i), at(i + 1)) < STRAIGHT_TURN;
  let run = 0;
  let straightM = 0;
  for (let i = 0; i < 2 * count; i += 1) {
    run = straightAt(i) || straightAt(i + 1) ? run + step(i) : 0;
    straightM = Math.max(straightM, Math.min(run, lapM));
  }
  return { lapM, tightestM, straightM };
}

/** A course the picker offers, with its sidecar's centreline for the preview's map. */
export interface CourseChoice {
  readonly course: Course;
  readonly centreline: readonly WorldPoint[];
}

export interface CoursePreviewOptions {
  /** Laps in a race when the card opens, the second of the four numbers. */
  readonly laps: number;
  /**
   * Whether the player asks for reduced motion, read every frame so a
   * change takes hold at once; while it does, the dot waits on the start line.
   */
  readonly reducedMotion: () => boolean;
}

export interface CoursePreview {
  /** The card; the caller puts it on the page. */
  readonly element: HTMLElement;
  /** Show `choice` in place of the last. */
  show(choice: CourseChoice): void;
  /** Show `laps` as the laps in a race, on this course and any shown next. */
  laps(laps: number): void;
  dispose(): void;
}

const MAP = { width: 380, height: 190, margin: 14 } as const;
const WALL_WIDTH = 11;
const ASPHALT_WIDTH = 7;
const START_TICK_HALF = 9;
const DOT_RADIUS = 5;
/** One lap of the dot, whatever the course's length. */
const LAP_MS = 8000;

/** A stat box: the number over what it counts. */
function stat(value: string, label: string): HTMLDivElement {
  const box = document.createElement("div");
  const number = document.createElement("b");
  number.textContent = value;
  const caption = document.createElement("small");
  caption.textContent = label;
  box.append(number, caption);
  return box;
}

function road(svg: SVGSVGElement, d: string, colour: number, width: number): void {
  const path = document.createElementNS(SVG_NS, "path");
  path.setAttribute("d", d);
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", hexCss(colour));
  path.setAttribute("stroke-width", String(width));
  path.setAttribute("stroke-linejoin", "round");
  svg.append(path);
}

/** The flat map of `centreline` with its start line, and the dot that laps it. */
function courseMap(course: Course, centreline: readonly WorldPoint[]) {
  const projection = projectCentreline(centreline, MAP, START_TICK_HALF, WALL_WIDTH);
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.classList.add("map");
  svg.setAttribute("viewBox", `0 0 ${MAP.width} ${MAP.height}`);
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", `${course.name} course map`);
  road(svg, projection.path, WALL, WALL_WIDTH);
  road(svg, projection.path, ASPHALT, ASPHALT_WIDTH);
  for (const deck of projection.decks) {
    road(svg, deck, WALL, WALL_WIDTH);
    road(svg, deck, ASPHALT, ASPHALT_WIDTH);
  }
  const [x1, y1, x2, y2] = projection.startTick;
  road(svg, `M ${x1} ${y1} L ${x2} ${y2}`, VALUE_WHITE, 3);
  const dot = document.createElementNS(SVG_NS, "circle");
  dot.setAttribute("r", String(DOT_RADIUS));
  dot.setAttribute("fill", hexCss(COIN_GOLD));
  dot.setAttribute("stroke", hexCss(INK));
  dot.setAttribute("stroke-width", "2");
  svg.append(dot);
  const points = centreline.map(([x, , z]) => projection.toMap(x, z));
  /** Put the dot `fraction` of the way round the lap. */
  const place = (fraction: number) => {
    const [cx, cy] = points[Math.floor(fraction * points.length) % points.length] ?? [0, 0];
    dot.setAttribute("cx", String(cx));
    dot.setAttribute("cy", String(cy));
  };
  place(0);
  return { svg, place };
}

/**
 * The preview card. Each `show` swaps the whole card, so the swap's entry
 * animation plays again, and restarts the dot on the start line.
 */
export function createCoursePreview(options: CoursePreviewOptions): CoursePreview {
  const element = document.createElement("section");
  element.className = "preview";
  element.setAttribute("aria-live", "polite");
  let frame = 0;
  let laps = options.laps;
  const lapsStat = document.createElement("b");
  return {
    element,
    laps(picked) {
      laps = picked;
      lapsStat.textContent = String(laps);
    },
    show({ course, centreline }) {
      cancelAnimationFrame(frame);
      const swap = document.createElement("div");
      swap.className = "map-swap";
      const header = document.createElement("header");
      header.className = "preview-head";
      const name = document.createElement("h3");
      name.textContent = course.name;
      const after = document.createElement("span");
      after.className = "after";
      after.textContent = `After ${course.after}`;
      header.append(name, after);
      const map = courseMap(course, centreline);
      const blurb = document.createElement("p");
      blurb.className = "blurb";
      blurb.textContent = course.blurb;
      const numbers = courseStats(centreline);
      lapsStat.textContent = String(laps);
      const lapsBox = stat("", "Laps");
      lapsBox.querySelector("b")?.replaceWith(lapsStat);
      const stats = document.createElement("div");
      stats.className = "stats";
      stats.append(
        stat(`${Math.round(numbers.lapM)} m`, "Lap"),
        lapsBox,
        stat(`${Math.round(numbers.tightestM)} m`, "Tightest"),
        stat(`${Math.round(numbers.straightM)} m`, "Straight"),
      );
      swap.append(header, map.svg, blurb, stats);
      element.replaceChildren(swap);
      let started: number | undefined;
      const run = (now: number) => {
        if (options.reducedMotion()) {
          started = undefined;
          map.place(0);
        } else {
          started ??= now;
          map.place(((now - started) / LAP_MS) % 1);
        }
        frame = requestAnimationFrame(run);
      };
      frame = requestAnimationFrame(run);
    },
    dispose() {
      cancelAnimationFrame(frame);
      element.remove();
    },
  };
}
