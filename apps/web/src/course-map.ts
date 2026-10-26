// The course map: the track seen from above with a marker per kart, part of
// the Instrument panel, drawn as the world builds the track: asphalt between
// white walls over the ink drop (ADR 0037). The outline is the sidecar's centreline projected
// onto the x/z plane, so the map is fixed, never rotates with the own kart
// and is never mirrored against the View: +x runs right and +z runs down,
// which is how the world looks from above with the grid facing up the map.

import type { KartReadout } from "./bridge";
import { hexCss, livery } from "./livery";
import { facetted, group, line, ngon, polygon, SVG_NS } from "./low-poly-svg";
import { ASPHALT, INK, tonesOf, VALUE_WHITE, WALL } from "./palette";
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
  /** Where a world `x`/`z` lands on the map. */
  toMap(x: number, z: number): [number, number];
}

/**
 * The top-down projection of `centreline` into `box`: uniformly scaled so
 * the outline fits inside the margin on its longer side and is centred on
 * the shorter, keeping the track's shape. Height is ignored. The start
 * tick reaches `startTickHalf` map units either side of the centreline,
 * so it pokes past the road however the track is scaled. Throws on an
 * empty centreline, which no sidecar has.
 */
export function projectCentreline(
  centreline: readonly WorldPoint[],
  box: MapBox,
  startTickHalf: number,
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

  return { path, startTick, toMap };
}

/** What the map is built from: the track's centreline and who is on the grid. */
export interface CourseMapSetup {
  readonly centreline: readonly WorldPoint[];
  readonly kartCount: number;
  /** The kart whose marker is the big ringed one. */
  readonly ownKart: number;
}

export interface CourseMap {
  /** The SVG, addressable by `data-instrument="course-map"`; the caller puts it on the page. */
  readonly element: SVGSVGElement;
  /** Move every marker to where `transforms` put its kart this frame. */
  update(transforms: readonly KartReadout[]): void;
}

/**
 * The map is drawn in a fixed square of map units and scaled by CSS, so
 * one projection serves every viewport and a resize costs nothing.
 */
const MAP_UNITS = 100;
const MAP_MARGIN = 8;
/** The asphalt's width in map units: thick enough to read as a road at a glance. */
const ASPHALT_WIDTH = 6;
/** The asphalt with a white wall either side. */
const WALL_WIDTH = 9;
/** How far the ink drop sits down and right of the track, and of each marker. */
const DROP_OFFSET = 1.6;
const MARKER_DROP_OFFSET = 1.2;
const DROP_OPACITY = 0.55;
/** The start's chequer: two rows of four squares across the walls. */
const CHEQUER = 2.4;
const START_TICK_HALF = CHEQUER * 2;
const MARKER_RADIUS = 3.6;
const OWN_RING_RADIUS = 6;
const OWN_MARKER_RADIUS = 4.4;

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
  );
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.dataset.instrument = "course-map";
  svg.setAttribute("viewBox", `0 0 ${MAP_UNITS} ${MAP_UNITS}`);

  const road = (colour: number, width: number) => {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", projection.path);
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
  road(WALL, WALL_WIDTH);
  road(ASPHALT, ASPHALT_WIDTH);

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
    facetted(marker, 8, 0, 0, own ? OWN_MARKER_RADIUS : MARKER_RADIUS, tonesOf(livery(kart).key));
    return marker;
  });
  // Document order is paint order: the own kart's marker goes on last.
  const own = markers[setup.ownKart];
  svg.append(...markers.filter((marker) => marker !== own));
  if (own !== undefined) {
    svg.append(own);
  }

  return {
    element: svg,
    update(transforms) {
      transforms.forEach((transform, kart) => {
        const marker = markers[kart];
        if (marker === undefined) {
          return;
        }
        const [u, v] = projection.toMap(transform.position[0], transform.position[2]);
        marker.setAttribute("transform", `translate(${u} ${v})`);
      });
    },
  };
}
