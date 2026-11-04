// The course map: the track seen from above with a marker per kart, part of
// the Instrument panel, drawn as the world builds the track: asphalt between
// white walls over the ink drop (ADR 0037). The outline is the sidecar's centreline projected
// onto the x/z plane, so the map is fixed, never rotates with the own kart
// and is never mirrored against the View: +x runs right and +z runs down,
// which is how the world looks from above with the grid facing up the map.

import type { KartReadout } from "./bridge";
import { hexCss, livery } from "./livery";
import { facets, facetted, group, line, ngon, polygon, SVG_NS, type Tone } from "./low-poly-svg";
import { ASPHALT, INK, type Tones, tonesOf, VALUE_WHITE, WALL } from "./palette";
import { STAR_STILL_SHINE, starColour } from "./star-shine";
import type { WorldPoint } from "./track";

/** The box the map is drawn into, in map units, with the margin the outline keeps from its edges. */
export interface MapBox {
  readonly width: number;
  readonly height: number;
  readonly margin: number;
}

export interface MapProjection {
  /** The outline: an SVG path through every centreline point, closed back to the first. */
  readonly path: string;
  /** The start/finish tick `[x1, y1, x2, y2]`: across the track at the first centreline point. */
  readonly startTick: readonly [number, number, number, number];
  /**
   * One open SVG path for each place the course crosses itself, along the
   * higher leg, so it can be drawn again over the leg it bridges.
   */
  readonly decks: readonly string[];
  /** Where a world `x`/`z` lands on the map. */
  toMap(x: number, z: number): [number, number];
}

/**
 * The top-down projection of `centreline` into `box`: uniformly scaled so
 * the outline fits inside the margin on its longer side and is centred on
 * the shorter, keeping the track's shape. Height is ignored but where the
 * course crosses itself, and only to tell which leg is the deck. The start
 * tick reaches `startTickHalf` map units either side of the centreline,
 * so it pokes past the road however the track is scaled; each deck reaches
 * `deckHalf` map units along its leg either side of the crossing. Throws on
 * an empty centreline, which no sidecar has.
 */
export function projectCentreline(
  centreline: readonly WorldPoint[],
  box: MapBox,
  startTickHalf: number,
  deckHalf: number,
): MapProjection {
  const [first, second] = centreline;
  if (first === undefined) {
    throw new RangeError("a course map needs a centreline");
  }
  let minX = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let minZ = Number.POSITIVE_INFINITY;
  let maxZ = Number.NEGATIVE_INFINITY;
  for (const [x, , z] of centreline) {
    minX = Math.min(minX, x);
    maxX = Math.max(maxX, x);
    minZ = Math.min(minZ, z);
    maxZ = Math.max(maxZ, z);
  }
  const innerWidth = box.width - 2 * box.margin;
  const innerHeight = box.height - 2 * box.margin;
  const scale = Math.min(innerWidth / (maxX - minX || 1), innerHeight / (maxZ - minZ || 1));
  const offsetU = box.margin + (innerWidth - (maxX - minX) * scale) / 2;
  const offsetV = box.margin + (innerHeight - (maxZ - minZ) * scale) / 2;
  const toMap = (x: number, z: number): [number, number] => [
    offsetU + (x - minX) * scale,
    offsetV + (z - minZ) * scale,
  ];

  const path = `${centreline
    .map(([x, , z], i) => `${i === 0 ? "M" : "L"} ${toMap(x, z).join(" ")}`)
    .join(" ")} Z`;

  // The tick runs perpendicular to the direction of travel off the line.
  const ahead = second ?? first;
  const dx = ahead[0] - first[0];
  const dz = ahead[2] - first[2];
  const length = Math.hypot(dx, dz) || 1;
  const acrossX = (-dz / length) * startTickHalf;
  const acrossZ = (dx / length) * startTickHalf;
  const [u, v] = toMap(first[0], first[2]);
  const startTick = [u - acrossX, v - acrossZ, u + acrossX, v + acrossZ] as const;

  const mapped = centreline.map(([x, , z]) => toMap(x, z));
  const decks = crossings(centreline).map((crossing) => deckPath(mapped, crossing, deckHalf));
  return { path, startTick, toMap, decks };
}

/** Where the course crosses itself: the higher leg's segment and how far along it the crossing lies. */
interface Crossing {
  readonly segment: number;
  readonly along: number;
}

/**
 * Every place two segments of the closed `centreline` cross in x/z, as a
 * point on whichever is higher there. Neighbouring segments share a point
 * and never count.
 */
function crossings(centreline: readonly WorldPoint[]): Crossing[] {
  const count = centreline.length;
  const at = (i: number) => centreline[i % count] as WorldPoint;
  const found: Crossing[] = [];
  for (let i = 0; i < count; i++) {
    const [ax, ay, az] = at(i);
    const [bx, by, bz] = at(i + 1);
    for (let j = i + 2; j < count; j++) {
      if (i === 0 && j === count - 1) {
        continue;
      }
      const [cx, cy, cz] = at(j);
      const [dx, dy, dz] = at(j + 1);
      const denominator = (bx - ax) * (dz - cz) - (bz - az) * (dx - cx);
      if (denominator === 0) {
        continue;
      }
      const t = ((cx - ax) * (dz - cz) - (cz - az) * (dx - cx)) / denominator;
      const u = ((cx - ax) * (bz - az) - (cz - az) * (bx - ax)) / denominator;
      if (t < 0 || t >= 1 || u < 0 || u >= 1) {
        continue;
      }
      const first = ay + (by - ay) * t;
      const second = cy + (dy - cy) * u;
      found.push(first >= second ? { segment: i, along: t } : { segment: j, along: u });
    }
  }
  return found;
}

/** An open path along `mapped` through `crossing`, `half` map units back and ahead of it. */
function deckPath(mapped: readonly [number, number][], crossing: Crossing, half: number): string {
  const count = mapped.length;
  const at = (i: number) => mapped[((i % count) + count) % count] as [number, number];
  const lerp = (a: [number, number], b: [number, number], t: number): [number, number] => [
    a[0] + (b[0] - a[0]) * t,
    a[1] + (b[1] - a[1]) * t,
  ];
  const centre = lerp(at(crossing.segment), at(crossing.segment + 1), crossing.along);
  // Walk from the crossing one way round the loop, collecting points until
  // `half` map units have been covered.
  const walk = (step: 1 | -1): [number, number][] => {
    const points: [number, number][] = [];
    let from = centre;
    let next = step === 1 ? crossing.segment + 1 : crossing.segment;
    let left = half;
    for (let guard = 0; guard < count; guard++) {
      const to = at(next);
      const length = Math.hypot(to[0] - from[0], to[1] - from[1]);
      if (length >= left) {
        points.push(lerp(from, to, left / length));
        return points;
      }
      points.push(to);
      left -= length;
      from = to;
      next += step;
    }
    return points;
  };
  const points = [...walk(-1).reverse(), centre, ...walk(1)];
  return points.map((point, i) => `${i === 0 ? "M" : "L"} ${point.join(" ")}`).join(" ");
}

/**
 * An open path along a pit lane's points on the map. A lane stands closer to
 * the centreline than the map's road is wide, so it is drawn further out, each
 * point scaled away from the road's nearest point so the lane's furthest stands
 * `beside` map units off the line: it still leaves and joins the road at its ends.
 */
export function pitLanePath(
  projection: MapProjection,
  centreline: readonly WorldPoint[],
  lane: readonly WorldPoint[],
  beside: number,
): string {
  const road = centreline.map(([x, , z]) => projection.toMap(x, z));
  const onTheMap = lane.map(([x, , z]) => {
    const point = projection.toMap(x, z);
    const near = nearestOn(road, point);
    return { near, away: [point[0] - near[0], point[1] - near[1]] as const };
  });
  const furthest = Math.max(...onTheMap.map(({ away }) => Math.hypot(away[0], away[1])));
  const scale = beside / (furthest || 1);
  return onTheMap
    .map(({ near, away }, i) => {
      const u = near[0] + away[0] * scale;
      const v = near[1] + away[1] * scale;
      return `${i === 0 ? "M" : "L"} ${u} ${v}`;
    })
    .join(" ");
}

/** The nearest point to `point` on the closed polyline `road`. */
function nearestOn(
  road: readonly (readonly [number, number])[],
  point: readonly [number, number],
): readonly [number, number] {
  let best: readonly [number, number] = point;
  let bestSquared = Number.POSITIVE_INFINITY;
  road.forEach((a, i) => {
    const b = road[(i + 1) % road.length] as readonly [number, number];
    const dx = b[0] - a[0];
    const dy = b[1] - a[1];
    const lengthSquared = dx * dx + dy * dy;
    const along =
      lengthSquared === 0 ? 0 : ((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / lengthSquared;
    const t = Math.min(1, Math.max(0, along));
    const u = a[0] + dx * t;
    const v = a[1] + dy * t;
    const squared = (point[0] - u) ** 2 + (point[1] - v) ** 2;
    if (squared < bestSquared) {
      bestSquared = squared;
      best = [u, v];
    }
  });
  return best;
}

/** What the map is built from: the track's centreline and pit lane, if any, and who is on the grid. */
export interface CourseMapSetup {
  readonly centreline: readonly WorldPoint[];
  readonly pitLane?: readonly WorldPoint[] | undefined;
  readonly kartCount: number;
  /** The kart whose marker is the big ringed one. */
  readonly ownKart: number;
}

export interface CourseMap {
  /** The SVG, addressable by `data-instrument="course-map"`; the caller puts it on the page. */
  readonly element: SVGSVGElement;
  /**
   * Move every marker to where `transforms` put its kart this frame, and
   * run a star kart's marker through the rainbow loop (ADR 0075), or hold
   * it white under `reducedMotion`.
   */
  update(transforms: readonly KartReadout[], reducedMotion?: boolean): void;
}

/**
 * The map is drawn in a fixed square of map units and scaled by CSS, so
 * one projection serves every viewport and a resize costs nothing.
 */
const MAP_UNITS = 100;
const MAP_MARGIN = 8;
/** The asphalt's width in map units: thick enough to read as a road at a glance. */
const ASPHALT_WIDTH = 6;
/** The width of the asphalt with a white wall either side. */
const WALL_WIDTH = 9;
/** How far the ink drop sits down and right of the track, and of each marker. */
const DROP_OFFSET = 1.6;
const MARKER_DROP_OFFSET = 1.2;
const DROP_OPACITY = 0.55;
/** The start's chequer: two rows of four squares across the walls. */
const CHEQUER = 2.4;
const START_TICK_HALF = CHEQUER * 2;
/** How far a deck reaches along its leg either side of the crossing: past the road it bridges. */
const DECK_HALF = WALL_WIDTH;
/** A thin line beside the road, its middle clear of the road's walls. */
const PIT_LANE_WIDTH = 1.5;
const PIT_LANE_BESIDE = WALL_WIDTH / 2 + PIT_LANE_WIDTH;
const MARKER_RADIUS = 3.6;
const OWN_RING_RADIUS = 6;
const OWN_MARKER_RADIUS = 4.4;

/** A marker's faces with the tone each takes from how it faces the light, as `facetted` drew them. */
interface MarkerFaces {
  readonly faces: readonly SVGPolygonElement[];
  readonly tones: readonly Tone[];
}

/** Paint a marker's faces in `tones`, each by its own tone. */
function paintFacets({ faces, tones: byFace }: MarkerFaces, tones: Tones): void {
  faces.forEach((face, index) => {
    const tone = byFace[index] ?? 1;
    const colour = hexCss([tones.lit, tones.mid, tones.shaded][tone] ?? tones.mid);
    face.setAttribute("fill", colour);
    face.setAttribute("stroke", colour);
  });
}

/**
 * The course map's element: the track and its chequered start once, and a
 * faceted octagon per kart in its key colour, the own kart's larger,
 * ringed in white and last in the document so it draws over the rest.
 */
export function createCourseMap(setup: CourseMapSetup): CourseMap {
  const projection = projectCentreline(
    setup.centreline,
    { width: MAP_UNITS, height: MAP_UNITS, margin: MAP_MARGIN },
    START_TICK_HALF,
    DECK_HALF,
  );
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.dataset.instrument = "course-map";
  svg.setAttribute("viewBox", `0 0 ${MAP_UNITS} ${MAP_UNITS}`);

  const road = (colour: number, width: number, d = projection.path) => {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", d);
    path.setAttribute("fill", "none");
    path.setAttribute("stroke", hexCss(colour));
    path.setAttribute("stroke-width", String(width));
    path.setAttribute("stroke-linejoin", "round");
    svg.append(path);
    return path;
  };
  const drop = road(INK, WALL_WIDTH);
  drop.setAttribute("stroke-opacity", String(DROP_OPACITY));
  drop.setAttribute("transform", `translate(${DROP_OFFSET} ${DROP_OFFSET})`);
  // Under the road, so its two ends tuck in where it leaves and joins it.
  if (setup.pitLane) {
    const d = pitLanePath(projection, setup.centreline, setup.pitLane, PIT_LANE_BESIDE);
    road(WALL, PIT_LANE_WIDTH, d).dataset.pitLane = "";
  }
  road(WALL, WALL_WIDTH);
  road(ASPHALT, ASPHALT_WIDTH);
  // Each deck again on top, so its walls break the road it passes over and
  // the map reads as a bridge rather than an X.
  for (const deck of projection.decks) {
    road(WALL, WALL_WIDTH, deck);
    road(ASPHALT, ASPHALT_WIDTH, deck);
  }

  // The chequer is two rows across the track, one each side of the tick,
  // each a white line with ink dashes, the second row's dashes shifted one square.
  const [x1, y1, x2, y2] = projection.startTick;
  const length = Math.hypot(x2 - x1, y2 - y1) || 1;
  const alongX = (-(y2 - y1) / length) * (CHEQUER / 2);
  const alongY = ((x2 - x1) / length) * (CHEQUER / 2);
  const start = group(svg);
  start.setAttribute("shape-rendering", "crispEdges");
  for (const [side, offset] of [
    [-1, "0"],
    [1, String(CHEQUER)],
  ] as const) {
    const a = [x1 + side * alongX, y1 + side * alongY] as const;
    const b = [x2 + side * alongX, y2 + side * alongY] as const;
    line(start, a, b, VALUE_WHITE, CHEQUER);
    const dashes = line(start, a, b, INK, CHEQUER);
    dashes.setAttribute("stroke-dasharray", `${CHEQUER} ${CHEQUER}`);
    dashes.setAttribute("stroke-dashoffset", offset);
  }

  const markers = Array.from({ length: setup.kartCount }, (_, kart) => {
    const marker = document.createElementNS(SVG_NS, "g");
    marker.dataset.kart = String(kart);
    const own = kart === setup.ownKart;
    const outer = own ? OWN_RING_RADIUS : MARKER_RADIUS;
    const shadow = polygon(marker, ngon(8, MARKER_DROP_OFFSET, MARKER_DROP_OFFSET, outer), INK);
    shadow.setAttribute("fill-opacity", String(DROP_OPACITY));
    if (own) {
      marker.dataset.own = "";
      polygon(marker, ngon(8, 0, 0, OWN_RING_RADIUS), VALUE_WHITE).dataset.ring = "";
    }
    const before = marker.childElementCount;
    const radius = own ? OWN_MARKER_RADIUS : MARKER_RADIUS;
    facetted(marker, 8, 0, 0, radius, tonesOf(livery(kart).key));
    // The faces facetted drew, in order, so the star's loop repaints them by
    // tone and the key colour comes back the same way.
    const drawnFaces: MarkerFaces = {
      faces: [...marker.children].slice(before) as SVGPolygonElement[],
      tones: facets(8, 0, 0, radius).map((face) => face.tone),
    };
    return { marker, facets: drawnFaces, key: tonesOf(livery(kart).key), shining: false };
  });
  // Document order is paint order: the own kart's marker goes on last.
  const own = markers[setup.ownKart];
  svg.append(...markers.filter((drawn) => drawn !== own).map((drawn) => drawn.marker));
  if (own !== undefined) {
    svg.append(own.marker);
  }

  return {
    element: svg,
    update(transforms, reducedMotion = false) {
      transforms.forEach((transform, kart) => {
        const drawn = markers[kart];
        if (drawn === undefined) {
          return;
        }
        const [u, v] = projection.toMap(transform.position[0], transform.position[2]);
        drawn.marker.setAttribute("transform", `translate(${u} ${v})`);
        if (transform.starS !== null) {
          const shine = reducedMotion ? STAR_STILL_SHINE : starColour(transform.starS);
          paintFacets(drawn.facets, tonesOf(shine));
          drawn.marker.dataset.star = "";
          drawn.shining = true;
        } else if (drawn.shining) {
          paintFacets(drawn.facets, drawn.key);
          delete drawn.marker.dataset.star;
          drawn.shining = false;
        }
      });
    },
  };
}
