// The poster kart's frame: build.py's metres at 250 units each in a
// 520 x 380 viewBox, the kart's origin at x 262 and its wheels on the
// ground line at y 350, as apps/web/src/poster.ts has always framed it, so
// the Scenes' rows and widths do not move. The camera is pitched down a few
// degrees so every top face shows as the lit strip the traced poster drew
// along each box, and the far wheels as the sliver of tyre an elevated side
// view gives.

import type { Model } from "./glb.ts";
import {
  order,
  type Plane,
  type Point,
  planes,
  project,
  projectPoint,
  type Tone,
  visible,
} from "./project.ts";

export const VIEW_BOX = "0 0 520 380";
export const UNITS_PER_M = 250;
export const ORIGIN_X = 262;
export const GROUND_Y = 350;
/** Tuned by eye against docs/mockups/poster-kart-generated.html. */
export const PITCH = (4 * Math.PI) / 180;
/** From above and behind the kart, so top faces light and faces towards the nose shade, as the traced poster lit them. */
export const LIGHT = [0, 1.3, 1] as const;

export interface SidePath {
  readonly node: string;
  readonly material: string;
  readonly tone: Tone;
  /** SVG path data in viewBox units, one subpath per outline. */
  readonly d: string;
}

export interface SideView {
  readonly viewBox: string;
  /** Every part of the kart, whether or not a face of it shows side on. */
  readonly nodes: readonly string[];
  /** In painter order: the first is furthest from the camera. */
  readonly paths: readonly SidePath[];
  /** Each wheel node's axle on the picture, in viewBox units. */
  readonly wheels: Readonly<Record<string, readonly [number, number]>>;
}

const sameFace = (a: Plane, b: Plane) =>
  a.node === b.node && a.material === b.material && a.tone === b.tone;

/** The parts a driver may or may not wear; everything else is always drawn. */
const optional = (node: string) =>
  node.startsWith("hair-") || node.startsWith("facial-hair-") || node === "earrings";

/** Two hair styles, or two facial hairs, are never worn together, so neither hides the other. */
const together = (a: string, b: string) =>
  a === b ||
  !(
    (a.startsWith("hair-") && b.startsWith("hair-")) ||
    (a.startsWith("facial-hair-") && b.startsWith("facial-hair-"))
  );

/** The poster kart's side view of `model`. */
export function sideView(model: Model): SideView {
  const facets = project(model.triangles, { pitch: PITCH, light: LIGHT });
  const ordered = order(planes(visible(facets, (node) => !optional(node))), together);
  const lowest = Math.min(...ordered.flatMap((plane) => plane.loops.flat().map(([, y]) => y)));
  // The frame: the kart's lowest point, its near wheels' contact, on the ground line.
  const toUnits = ([x, y]: Point): [number, number] => [
    ORIGIN_X + x * UNITS_PER_M,
    GROUND_Y - (y - lowest) * UNITS_PER_M,
  ];
  const fixed = (value: number) => value.toFixed(1);
  const loopData = (loop: readonly Point[]) =>
    `${loop.map((point, index) => `${index === 0 ? "M" : "L"}${toUnits(point).map(fixed).join(" ")}`).join("")}Z`;
  const paths: SidePath[] = [];
  for (const plane of ordered) {
    const last = paths[paths.length - 1];
    const d = plane.loops.map(loopData).join("");
    if (last !== undefined && sameFace(plane, last as unknown as Plane)) {
      paths[paths.length - 1] = { ...last, d: last.d + d };
    } else {
      paths.push({ node: plane.node, material: plane.material, tone: plane.tone, d });
    }
  }
  const wheels: Record<string, readonly [number, number]> = {};
  for (const [name, origin] of model.origins) {
    if (!name.startsWith("wheel-")) continue;
    const [x, y] = toUnits(projectPoint(origin, PITCH).at);
    wheels[name] = [Number(fixed(x)), Number(fixed(y))];
  }
  return { viewBox: VIEW_BOX, nodes: model.meshed, paths, wheels };
}
