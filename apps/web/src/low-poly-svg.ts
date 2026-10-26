// Polygons for the Instrument panel's inline SVG (ADR 0037), drawn the way
// the catalog draws them: regular polygons flat side down, and facets toned
// by how each edge faces a light from the top left, as the View's sun lights
// the wheel hubs.

import { hexCss } from "./livery";
import type { Tones } from "./palette";

export const SVG_NS = "http://www.w3.org/2000/svg";

export type Point = readonly [number, number];

/** Which of a facet's tones a face takes: 0 lit, 1 mid, 2 shaded. */
export type Tone = 0 | 1 | 2;

/** A regular `sides`-gon of circumradius `r` around `cx`, `cy`, flat side down. */
export function ngon(sides: number, cx: number, cy: number, r: number): Point[] {
  return Array.from({ length: sides }, (_, k) => {
    const t = Math.PI / sides + (2 * Math.PI * k) / sides - Math.PI / 2;
    return [cx + r * Math.cos(t), cy + r * Math.sin(t)] as const;
  });
}

const LIGHT = (() => {
  const length = Math.hypot(-1, -1.3);
  return [-1 / length, -1.3 / length] as const;
})();

/** The corners of the rectangle from `x1`, `y1` to `x2`, `y2`. */
export function rectangle(x1: number, y1: number, x2: number, y2: number): Point[] {
  return [
    [x1, y1],
    [x2, y1],
    [x2, y2],
    [x1, y2],
  ];
}

/** The `ngon` cut into a triangle per edge, each with the tone its edge faces the light with. */
export function facets(
  sides: number,
  cx: number,
  cy: number,
  r: number,
): { readonly points: Point[]; readonly tone: Tone }[] {
  const rim = ngon(sides, cx, cy, r);
  return rim.map((a, i) => {
    const b = rim[(i + 1) % sides] ?? a;
    const mx = (a[0] + b[0]) / 2 - cx;
    const my = (a[1] + b[1]) / 2 - cy;
    const length = Math.hypot(mx, my) || 1;
    const dot = (mx / length) * LIGHT[0] + (my / length) * LIGHT[1];
    const tone: Tone = dot > 0.3 ? 0 : dot < -0.3 ? 2 : 1;
    return { points: [[cx, cy], a, b], tone };
  });
}

/** An empty SVG on `viewBox`, decorative, as every panel drawing is: the readout's text says it. */
export function svg(viewBox: string): SVGSVGElement {
  const element = document.createElementNS(SVG_NS, "svg");
  element.setAttribute("viewBox", viewBox);
  element.setAttribute("aria-hidden", "true");
  return element;
}

export function group(parent: Element): SVGGElement {
  const element = document.createElementNS(SVG_NS, "g");
  parent.append(element);
  return element;
}

/** A polygon through `points` on `parent`, filled with `fill` when given; otherwise it inherits. */
export function polygon(
  parent: Element,
  points: readonly Point[],
  fill?: number | string,
): SVGPolygonElement {
  const element = document.createElementNS(SVG_NS, "polygon");
  element.setAttribute("points", points.map(([x, y]) => `${round(x)},${round(y)}`).join(" "));
  if (fill !== undefined)
    element.setAttribute("fill", typeof fill === "number" ? hexCss(fill) : fill);
  parent.append(element);
  return element;
}

/**
 * The facets of an `ngon` on `parent`. Filled from `tones` when given;
 * otherwise each face is marked `data-tone` so a style picks its colour,
 * as the shift lights and the tyres do by state.
 */
export function facetted(
  parent: Element,
  sides: number,
  cx: number,
  cy: number,
  r: number,
  tones?: Tones,
): void {
  for (const { points, tone } of facets(sides, cx, cy, r)) {
    const colour = tones === undefined ? undefined : [tones.lit, tones.mid, tones.shaded][tone];
    const face = polygon(parent, points, colour);
    // A hairline of the face's own colour closes the seams between facets;
    // a toned face takes its stroke from the same style as its fill.
    face.setAttribute("stroke-width", "0.6");
    face.setAttribute("stroke-linejoin", "round");
    if (colour === undefined) {
      face.dataset.tone = String(tone);
    } else {
      face.setAttribute("stroke", hexCss(colour));
    }
  }
}

/** A line on `parent` from `a` to `b` in `stroke`, `width` wide. */
export function line(
  parent: Element,
  a: Point,
  b: Point,
  stroke: number,
  width: number,
): SVGLineElement {
  const element = document.createElementNS(SVG_NS, "line");
  element.setAttribute("x1", String(round(a[0])));
  element.setAttribute("y1", String(round(a[1])));
  element.setAttribute("x2", String(round(b[0])));
  element.setAttribute("y2", String(round(b[1])));
  element.setAttribute("stroke", hexCss(stroke));
  element.setAttribute("stroke-width", String(width));
  parent.append(element);
  return element;
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
