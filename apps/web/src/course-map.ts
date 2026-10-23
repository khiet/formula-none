// The course map: the track seen from above with a dot per kart, part of
// the Instrument panel. The outline is the sidecar's centreline projected
// onto the x/z plane, so the map is fixed, never rotates with the own kart
// and is never mirrored against the View: +x runs right and +z runs down,
// which is how the world looks from above with the grid facing up the map.

import type { KartTransform } from "./bridge";
import { liveryCss } from "./livery";
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
  update(transforms: readonly KartTransform[]): void;
}

/**
 * The map is drawn in a fixed square of map units and scaled by CSS, so
 * one projection serves every viewport and a resize costs nothing.
 */
const MAP_UNITS = 100;
const MAP_MARGIN = 8;
/** The outline's width in map units: thick enough to read as a road at a glance. */
const OUTLINE_WIDTH = 6;
/** The tick reaches past the road's edges so it reads against the outline. */
const START_TICK_HALF = OUTLINE_WIDTH * 0.8;
const START_TICK_WIDTH = 2;
const MARKER_RADIUS = 3;
const OWN_MARKER_RADIUS = 4.5;
const OWN_RING_WIDTH = 1.5;

const SVG_NS = "http://www.w3.org/2000/svg";

/**
 * The course map's element: the outline and the start tick once, and a
 * marker per kart in its livery, the own kart's larger, ringed in white and
 * last in the document so it draws over the rest.
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

  const outline = document.createElementNS(SVG_NS, "path");
  outline.setAttribute("d", projection.path);
  outline.setAttribute("fill", "none");
  outline.setAttribute("stroke", "rgba(255, 255, 255, 0.55)");
  outline.setAttribute("stroke-width", String(OUTLINE_WIDTH));
  outline.setAttribute("stroke-linejoin", "round");
  outline.setAttribute("stroke-linecap", "round");

  const [x1, y1, x2, y2] = projection.startTick;
  const start = document.createElementNS(SVG_NS, "line");
  start.setAttribute("x1", String(x1));
  start.setAttribute("y1", String(y1));
  start.setAttribute("x2", String(x2));
  start.setAttribute("y2", String(y2));
  start.setAttribute("stroke", "#fff");
  start.setAttribute("stroke-width", String(START_TICK_WIDTH));
  svg.append(outline, start);

  const markers = Array.from({ length: setup.kartCount }, (_, kart) => {
    const marker = document.createElementNS(SVG_NS, "circle");
    marker.dataset.kart = String(kart);
    marker.setAttribute("fill", liveryCss(kart));
    const own = kart === setup.ownKart;
    marker.setAttribute("r", String(own ? OWN_MARKER_RADIUS : MARKER_RADIUS));
    if (own) {
      marker.dataset.own = "";
      marker.setAttribute("stroke", "#fff");
      marker.setAttribute("stroke-width", String(OWN_RING_WIDTH));
    }
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
        marker.setAttribute("cx", String(u));
        marker.setAttribute("cy", String(v));
      });
    },
  };
}
