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
const TYRE = ["#2f3237", "#1d1e21", "#111214"] as const;

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

/** An octagon tyre with three spokes and the hub in the accent. */
function wheel(cx: number, cy: number, r: number): string {
  const spokes = [0, 1, 2]
    .map((k) => {
      const t = -Math.PI / 2 + (2 * Math.PI * k) / 3 + 0.35;
      const x = (cx + r * 0.86 * Math.cos(t)).toFixed(1);
      const y = (cy + r * 0.86 * Math.sin(t)).toFixed(1);
      return `<line x1="${cx}" y1="${cy}" x2="${x}" y2="${y}" stroke="#4a4e55" stroke-width="${(r * 0.13).toFixed(1)}" stroke-linecap="round"/>`;
    })
    .join("");
  return (
    facets(8, cx, cy, r, TYRE) +
    poly(ngon(8, cx, cy, r * 0.58), "#3a3d42") +
    spokes +
    poly(ngon(6, cx, cy, r * 0.16), "var(--a)")
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

/**
 * The kart side on, facing right, as the style guide draws it: facets in
 * the livery's primary, secondary and accent, octagon wheels, and its
 * driver bare-headed. The colours ride on the SVG as custom properties;
 * index.html derives their lit and shaded tones.
 */
export function kartSide({ primary, secondary, accent, driver }: Livery): SVGSVGElement {
  const template = document.createElement("template");
  template.innerHTML = `<svg class="kart" viewBox="0 0 520 260" aria-hidden="true">
    <ellipse cx="270" cy="240" rx="225" ry="9" fill="rgb(0 0 0 / 0.18)"/>
    <polygon points="54,64 68,64 68,184 54,184" fill="var(--s)"/>
    <polygon points="40,60 132,60 128,82 44,82" fill="var(--p)"/>
    <polygon points="40,60 132,60 131,68 41,68" fill="var(--p-lt)"/>
    <polygon points="44,86 126,86 122,94 48,94" fill="var(--a)"/>
    <polygon points="70,196 70,150 100,124 190,116 222,92 262,92 292,118 400,140 478,168 482,190 460,196" fill="var(--p)"/>
    <polygon points="100,124 190,116 222,92 262,92 292,118 400,140 478,168 470,174 400,150 292,128 190,126 100,134 70,150" fill="var(--p-lt)"/>
    <polygon points="70,184 460,186 482,190 460,196 70,196" fill="var(--p-dk)"/>
    <polygon points="80,194 472,194 468,204 84,204" fill="#1d1d1f"/>
    <polygon points="150,188 158,146 200,134 330,138 372,162 362,188" fill="var(--s-dk)"/>
    <polygon points="158,146 200,134 330,138 372,162 330,148 200,144" fill="var(--s-lt)"/>
    <polygon points="176,162 372,162 440,174 440,178 372,168 176,168" fill="var(--a)"/>
    <polygon points="196,120 222,104 282,106 292,120" fill="#111"/>
    <polygon points="204,120 222,100 276,100 292,120" fill="var(--p)"/>
    <polygon points="204,120 222,100 248,100 240,120" fill="var(--p-lt)"/>
    <polygon points="226,102 272,102 268,94 230,94" fill="var(--a)"/>
    <polygon points="240,96 262,96 262,86 240,86" fill="var(--sk-dk)"/>
    ${head(driver, 252, 50, 42)}
    <polyline points="210,116 222,96 290,92 304,118" stroke="#1a1a1a" stroke-width="7" fill="none" stroke-linejoin="round" stroke-linecap="round"/>
    <polygon points="440,202 506,202 506,188 496,188 490,196 440,196" fill="var(--s)"/>
    <polygon points="498,174 506,174 506,204 498,204" fill="var(--a)"/>
    <line x1="130" y1="190" x2="190" y2="170" stroke="#222" stroke-width="3"/>
    <line x1="420" y1="198" x2="380" y2="170" stroke="#222" stroke-width="3"/>
    ${wheel(130, 190, 47)}${wheel(420, 198, 39)}
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
