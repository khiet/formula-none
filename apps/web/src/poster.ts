// The Grand Prix poster the Title and Results Scenes are set on: rotating
// rays and chequered bands behind them, and the kart side on. All of it is
// decoration, hidden from assistive technology; index.html styles it.

import { type Driver, type FacialHair, type Hair, hexCss, type Livery } from "./livery";

/** The rays and the chequered bands, to go first in a Scene so they sit behind it. */
export function backdrop(): HTMLElement[] {
  return ["rays", "chequer-top", "chequer-bottom"].map((part) => {
    const element = document.createElement("div");
    element.className = part;
    element.setAttribute("aria-hidden", "true");
    return element;
  });
}

// The kart's fixed colours, the same on every livery.
const IRIS = "#21170f";
const WHITES = "#f6f1ea";
const EARRING = "#dba833";
const GREYING = "#a8a8ad";
// The wheel and the carbon parts as build.py colours them.
const TYRE = ["#2f3237", "#1d1e21", "#111214"] as const;
const RIM = "#3a3d42";
const SPOKE = "#4a4e55";
const CARBON = "#2b2e33";

type Point = readonly [number, number];
type Tones = readonly [lit: string, mid: string, shaded: string];

/** A regular n-gon, flat side down, as the wheels and the head are cut. */
function ngon(sides: number, cx: number, cy: number, r: number): Point[] {
  return Array.from({ length: sides }, (_, k) => {
    const t = Math.PI / sides + (2 * Math.PI * k) / sides - Math.PI / 2;
    return [cx + r * Math.cos(t), cy + r * Math.sin(t)];
  });
}

const points = (corners: readonly Point[]): string =>
  corners.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");

const poly = (corners: readonly Point[], fill: string, extra = ""): string =>
  `<polygon points="${points(corners)}" fill="${fill}" ${extra}/>`;

const LIGHT: Point = [-1 / Math.hypot(1, 1.3), -1.3 / Math.hypot(1, 1.3)];

/** Flat-shaded facets: a triangle per edge, toned by how its edge faces a light from the top left. */
function facets(sides: number, cx: number, cy: number, r: number, tones: Tones): string {
  const corners = ngon(sides, cx, cy, r);
  return corners
    .map((a, i) => {
      const b = corners[(i + 1) % sides] ?? a;
      const mx = (a[0] + b[0]) / 2 - cx;
      const my = (a[1] + b[1]) / 2 - cy;
      const facing = (mx * LIGHT[0] + my * LIGHT[1]) / (Math.hypot(mx, my) || 1);
      const tone = facing > 0.3 ? tones[0] : facing < -0.3 ? tones[2] : tones[1];
      return poly([[cx, cy], a, b], tone, `stroke="${tone}" stroke-width="0.6"`);
    })
    .join("");
}

/** Places a point by its offset from a centre in units of a radius. */
const around =
  (cx: number, cy: number, r: number) =>
  (u: number, w: number): Point => [cx + u * r, cy + w * r];

/**
 * The wheel as build.py builds it: an octagon tyre, the rim an octagon at
 * 0.58 of it, and three spokes out to 0.86, one straight up, so a turn
 * shows every 120 degrees. In a group about its own centre for a style
 * to spin: `.wheel` with `transform-box: fill-box`.
 */
function wheel(cx: number, cy: number, r: number): string {
  const spokes = [0, 1, 2]
    .map((k) => {
      const t = -Math.PI / 2 + (2 * Math.PI * k) / 3;
      const x = (cx + r * 0.86 * Math.cos(t)).toFixed(1);
      const y = (cy + r * 0.86 * Math.sin(t)).toFixed(1);
      return `<line x1="${cx}" y1="${cy}" x2="${x}" y2="${y}" stroke="${SPOKE}" stroke-width="${(r * 0.2).toFixed(1)}" stroke-linecap="round"/>`;
    })
    .join("");
  return (
    `<g class="wheel">` +
    facets(8, cx, cy, r, TYRE) +
    poly(ngon(8, cx, cy, r * 0.58), RIM) +
    spokes +
    poly(ngon(6, cx, cy, r * 0.14), SPOKE) +
    "</g>"
  );
}

const HAIR_TONES: Tones = ["var(--hair-lt)", "var(--hair)", "var(--hair-dk)"];

/** Hair from the side, the kart facing right: a cap over the top and back, then the style's own shape. */
function hair(style: Hair, cx: number, cy: number, r: number): string {
  const at = around(cx, cy, r);
  const rim = ngon(8, cx, cy, r * 1.07);
  // The rim's corners from the back of the head over the crown.
  const crown = [5, 6, 7, 0].map((k): Point => rim[k] ?? [cx, cy]);
  let out = poly(
    [
      ...crown,
      at(0.74, -0.6),
      at(0.8, -0.44),
      at(0.3, -0.52),
      at(-0.28, -0.26),
      at(-0.48, 0.3),
      at(-0.84, 0.44),
    ],
    "var(--hair)",
  );
  out += poly(
    [...crown.slice(2), at(0.74, -0.6), at(0.3, -0.52), at(-0.1, -0.66)],
    "var(--hair-lt)",
  );
  switch (style) {
    case "braids-tied-back":
      out += [-0.5, -0.2, 0.1]
        .map(
          (u) =>
            `<polyline points="${points([at(u + 0.3, -0.96), at(u - 0.25, -0.5), at(u - 0.55, 0.05)])}" stroke="var(--hair-dk)" stroke-width="2.5" fill="none"/>`,
        )
        .join("");
      out += facets(6, cx - r * 1.05, cy - r * 0.15, r * 0.27, HAIR_TONES);
      break;
    case "swept-side-part":
      out += poly(
        [at(-0.2, -1.04), at(0.5, -1.12), at(0.94, -0.62), at(0.5, -0.56)],
        "var(--hair-lt)",
      );
      break;
    case "swept-back":
      out += poly(
        [at(-0.2, -1.12), at(0.62, -0.96), at(0.3, -0.56), at(-0.9, -0.62), at(-1.12, -0.18)],
        "var(--hair)",
      );
      out += poly([at(0.1, -1.04), at(0.5, -0.94), at(-0.5, -0.66)], "var(--hair-lt)");
      break;
    case "curly-fringe":
    case "short-curls": {
      const curls: readonly Point[] =
        style === "curly-fringe"
          ? [
              [0.62, -0.62],
              [0.4, -0.82],
              [0.12, -0.96],
              [-0.18, -0.98],
              [0.78, -0.42],
            ]
          : [
              [0.5, -0.74],
              [0.18, -0.92],
              [-0.16, -0.94],
              [-0.48, -0.78],
              [-0.76, -0.5],
              [-0.9, -0.12],
              [0.1, -0.62],
              [-0.4, -0.5],
            ];
      out += curls
        .map(([u, w]) => poly(ngon(6, cx + u * r, cy + w * r, r * 0.17), "var(--hair-lt)"))
        .join("");
      break;
    }
    case "short-greying-temples":
      out += poly([at(-0.34, -0.2), at(0.02, -0.3), at(0.06, 0), at(-0.28, 0.06)], GREYING);
      break;
    case "short":
      break;
  }
  return out;
}

const FACIAL_HAIR_OPACITY: Record<FacialHair, number> = {
  none: 0,
  "light-stubble": 0.18,
  stubble: 0.3,
  "heavy-stubble": 0.5,
  "short-beard": 0.92,
};

function facialHair(kind: FacialHair, cx: number, cy: number, r: number): string {
  const at = around(cx, cy, r);
  if (FACIAL_HAIR_OPACITY[kind] === 0) return "";
  return poly(
    [
      at(-0.08, 0.28),
      at(0.2, 0.64),
      at(0.6, 0.88),
      at(0.9, 0.62),
      at(0.92, 0.42),
      at(0.72, 0.66),
      at(0.3, 0.6),
      at(0.04, 0.24),
    ],
    "var(--hair)",
    `opacity="${FACIAL_HAIR_OPACITY[kind]}"`,
  );
}

/** The bare head (ADR 0036) from the side, facing right. */
function head(driver: Driver, cx: number, cy: number, r: number): string {
  const at = around(cx, cy, r);
  return (
    facets(8, cx, cy, r, ["var(--sk-lt)", "var(--sk)", "var(--sk-dk)"]) +
    facialHair(driver.facialHair, cx, cy, r) +
    hair(driver.hair, cx, cy, r) +
    poly(ngon(6, cx - r * 0.12, cy + r * 0.12, r * 0.17), "var(--sk-dk)") +
    (driver.earrings ? poly(ngon(4, cx - r * 0.12, cy + r * 0.38, r * 0.08), EARRING) : "") +
    poly([at(0.92, 0.04), at(1.1, 0.24), at(0.92, 0.3)], "var(--sk-dk)") +
    poly(ngon(6, cx + r * 0.6, cy + r * 0.04, r * 0.25), WHITES) +
    poly(ngon(6, cx + r * 0.7, cy + r * 0.06, r * 0.15), IRIS) +
    poly(ngon(4, cx + r * 0.74, cy - r * 0.02, r * 0.05), "#fff") +
    poly([at(0.36, -0.3), at(0.86, -0.26), at(0.84, -0.18), at(0.36, -0.22)], "var(--hair)") +
    `<polyline points="${points([at(0.6, 0.52), at(0.82, 0.5)])}" stroke="#6e3428" stroke-width="3" stroke-linecap="round"/>`
  );
}

// The side view's scale: build.py's metres to units of the 520 x 380 viewBox,
// the kart's y = 0 at ORIGIN_X and the ground at GROUND_Y.
const UNITS_PER_M = 250;
const ORIGIN_X = 262;
const GROUND_Y = 350;
/** How deep the lit strip along a box's top edge is, standing for its top face. */
const LIT_STRIP = 6;

const sx = (y: number): number => ORIGIN_X + y * UNITS_PER_M;
const sy = (z: number): number => GROUND_Y - z * UNITS_PER_M;

/**
 * The side of one of build.py's boxes, given as `box` gives it there: the
 * centre and size in metres (x, y, z), the front end's height scaled by
 * `frontScale` and lowered by `frontDrop`. Only y and z show side on; x is
 * kept so each part reads as the line in build.py. A lit strip runs along
 * the top edge when `lit` is given.
 */
function box(
  fill: string,
  [, cy, cz]: readonly [number, number, number],
  [, length, height]: readonly [number, number, number],
  frontScale = 1,
  frontDrop = 0,
  lit?: string,
): string {
  const back = sx(cy - length / 2);
  const front = sx(cy + length / 2);
  const backTop = sy(cz + height / 2);
  const frontTop = sy(cz + (height / 2) * frontScale - frontDrop);
  const side = poly(
    [
      [back, sy(cz - height / 2)],
      [back, backTop],
      [front, frontTop],
      [front, sy(cz - (height / 2) * frontScale - frontDrop)],
    ],
    fill,
  );
  if (lit === undefined) return side;
  return (
    side +
    poly(
      [
        [back, backTop],
        [front, frontTop],
        [front, frontTop + LIT_STRIP],
        [back, backTop + LIT_STRIP],
      ],
      lit,
    )
  );
}

/** A wheel's centre and radius from build.py's spec: the octagon rests on a flat, so its axle stands its inradius up. */
function wheelAt({ radius, y }: { radius: number; y: number }): [number, number, number] {
  return [sx(y), sy(radius * Math.cos(Math.PI / 8)), radius * UNITS_PER_M];
}

/**
 * The kart side on, facing right: build.py's car projected onto its side,
 * part for part in the same metres, painted in the livery's primary,
 * secondary and accent as the View paints it, its octagon wheels and its
 * driver bare-headed. The parts go down far side first, so each nearer one
 * covers what it hides in the View. The colours ride on the SVG as custom
 * properties; index.html derives their lit and shaded tones.
 */
export function kartSide({ primary, secondary, accent, driver }: Livery): SVGSVGElement {
  const p = "var(--p)";
  const s = "var(--s)";
  const a = "var(--a)";
  const body =
    box(a, [0, -0.6, 0.6], [0.03, 0.34, 0.16], 0.5, 0, "var(--a-lt)") +
    box(CARBON, [0, -0.78, 0.6], [0.05, 0.06, 0.36]) +
    box(a, [0, 0.81, 0.14], [0.13, 0.1, 0.09], 0.8) +
    box("var(--sk)", [0, -0.1, 0.77], [0.13, 0.12, 0.08]) +
    box(s, [0, -0.36, 0.62], [0.18, 0.14, 0.16], 1, 0, "var(--s-lt)") +
    box(a, [0, -0.1, 0.735], [0.24, 0.18, 0.05]) +
    box(p, [0, 0.52, 0.27], [0.26, 0.46, 0.26], 0.55, 0.07) +
    box(p, [0, -0.52, 0.34], [0.32, 0.44, 0.42], 1.1, 0, "var(--p-lt)") +
    box(p, [0, -0.04, 0.32], [0.42, 0.66, 0.4], 1, 0, "var(--p-lt)") +
    box(p, [0, -0.12, 0.62], [0.46, 0.24, 0.2], 1, 0, "var(--p-lt)") +
    head(driver, sx(-0.1), sy(0.99), 0.29 * UNITS_PER_M) +
    box(s, [0, -0.2, 0.1], [0.76, 1.1, 0.06]) +
    box(s, [0.29, -0.18, 0.28], [0.2, 0.52, 0.3], 0.75, 0.03, "var(--s-lt)") +
    // The halo: the pillar up from the nose, the hoop at chin height, the leg down to the tub.
    `<polyline points="${points([
      [sx(0.4), sy(0.52)],
      [sx(0.34), sy(0.756)],
      [sx(-0.1), sy(0.756)],
      [sx(-0.18), sy(0.52)],
    ])}" stroke="${CARBON}" stroke-width="11" fill="none" stroke-linejoin="round" stroke-linecap="round"/>` +
    box(p, [0, -0.82, 0.8], [0.82, 0.14, 0.06]) +
    box(a, [0, -0.86, 0.87], [0.82, 0.08, 0.04]) +
    box(a, [0.42, -0.83, 0.73], [0.03, 0.18, 0.34], 1, 0, "var(--a-lt)") +
    box(s, [0, 0.82, 0.07], [1.1, 0.14, 0.04]) +
    box(p, [0, 0.77, 0.12], [1.0, 0.06, 0.04]) +
    box(a, [0.56, 0.82, 0.11], [0.03, 0.16, 0.13], 1, 0, "var(--a-lt)") +
    wheel(...wheelAt({ radius: 0.3, y: -0.52 })) +
    wheel(...wheelAt({ radius: 0.27, y: 0.52 }));
  const template = document.createElement("template");
  template.innerHTML = `<svg class="kart" viewBox="0 0 520 380" aria-hidden="true">
    <ellipse cx="260" cy="352" rx="232" ry="9" fill="rgb(0 0 0 / 0.18)"/>
    ${body}
  </svg>`;
  const kart = template.content.firstElementChild as SVGSVGElement;
  for (const [name, colour] of [
    ["p", primary],
    ["s", secondary],
    ["a", accent],
    ["sk", driver.skin],
    ["hair", driver.hairColour],
  ] as const) {
    kart.style.setProperty(`--${name}`, hexCss(colour));
  }
  return kart;
}
