import type { Item } from "./generated/protocol";
import { hexCss } from "./livery";
import { facetted, line, ngon, type Point, polygon, SVG_NS, svg } from "./low-poly-svg";
import {
  GREYS,
  GULL_EYE,
  HAZARD_ORANGE,
  INK,
  ROCKET,
  ROCKET_FLAME,
  ROCKET_STRAP,
  ROCKET_WINDOW,
  tonesOf,
  WHITE,
} from "./palette";

/**
 * The item slot's characters (ADR 0037): each item drawn flat from the
 * View's mesh for it, in polygons lifted from the catalog's Aligned cards
 * (docs/mockups/asset-catalog.html). The snail has no mesh, since a boost
 * never shows on the road. Original work, CC0 like the rest of the art
 * (assets/LICENSES.md).
 */

type Drawing = (parent: SVGSVGElement) => void;

/** A stroked polyline through `points`, for the faces' brows and mouths. */
function stroke(parent: Element, points: readonly Point[], colour: number, width: number): void {
  const element = document.createElementNS(SVG_NS, "polyline");
  element.setAttribute("points", points.map((point) => point.join(",")).join(" "));
  element.setAttribute("stroke", hexCss(colour));
  element.setAttribute("stroke-width", String(width));
  element.setAttribute("fill", "none");
  parent.append(element);
}

/** The soft ground shadow every character stands on, as on the world cards. */
function shadow(parent: Element, cx: number, cy: number, rx: number, ry: number): void {
  const element = document.createElementNS(SVG_NS, "ellipse");
  for (const [name, value] of Object.entries({ cx, cy, rx, ry })) {
    element.setAttribute(name, String(value));
  }
  element.setAttribute("fill", hexCss(INK));
  element.setAttribute("fill-opacity", "0.15");
  parent.append(element);
}

const lerp = (a: Point, b: Point, t: number): Point => [
  a[0] + (b[0] - a[0]) * t,
  a[1] + (b[1] - a[1]) * t,
];

/** A traffic cone that hates you: six faceted sides, a white band, V brows and a flat mouth. */
const hazard: Drawing = (parent) => {
  shadow(parent, 100, 146, 54, 6);
  polygon(
    parent,
    [
      [52, 124],
      [72, 112],
      [128, 112],
      [148, 124],
    ],
    GREYS.grey,
  );
  polygon(
    parent,
    [
      [52, 124],
      [148, 124],
      [148, 138],
      [52, 138],
    ],
    GREYS.dark,
  );
  polygon(
    parent,
    [
      [148, 124],
      [128, 112],
      [128, 116],
      [148, 128],
    ],
    GREYS.deep,
  );
  const apex: Point = [100, 26];
  const rim: Point[] = [
    [62, 118],
    [82, 126],
    [118, 126],
    [138, 118],
  ];
  const sides = [HAZARD_ORANGE.lit, HAZARD_ORANGE.mid, HAZARD_ORANGE.shaded];
  const band = [WHITE.lit, WHITE.mid, WHITE.shaded];
  sides.forEach((fill, side) => {
    const a = rim[side] ?? apex;
    const b = rim[side + 1] ?? apex;
    polygon(parent, [apex, a, b], fill);
    polygon(
      parent,
      [lerp(apex, a, 0.5), lerp(apex, b, 0.5), lerp(apex, b, 0.66), lerp(apex, a, 0.66)],
      band[side],
    );
  });
  for (const x of [90, 105]) {
    polygon(
      parent,
      [
        [x, 75],
        [x + 5, 75],
        [x + 5, 81],
        [x, 81],
      ],
      INK,
    );
  }
  stroke(
    parent,
    [
      [87, 66],
      [97, 71],
    ],
    INK,
    3.5,
  );
  stroke(
    parent,
    [
      [113, 66],
      [103, 71],
    ],
    INK,
    3.5,
  );
  stroke(
    parent,
    [
      [93, 99],
      [107, 99],
    ],
    INK,
    3.5,
  );
};

/** A seagull with its eye locked on: white body, grey wing up, orange beak forward. */
const homing: Drawing = (parent) => {
  shadow(parent, 100, 146, 56, 5);
  polygon(
    parent,
    [
      [26, 82],
      [52, 90],
      [62, 104],
      [34, 102],
    ],
    GREYS.dark,
  );
  polygon(
    parent,
    [
      [26, 82],
      [52, 90],
      [40, 92],
    ],
    GREYS.grey,
  );
  polygon(
    parent,
    [
      [50, 90],
      [82, 70],
      [132, 68],
      [150, 82],
      [140, 102],
      [92, 110],
      [62, 106],
    ],
    GREYS.body,
  );
  polygon(
    parent,
    [
      [50, 90],
      [82, 70],
      [132, 68],
      [150, 82],
      [96, 88],
    ],
    GREYS.white,
  );
  polygon(
    parent,
    [
      [84, 74],
      [104, 24],
      [122, 22],
      [114, 72],
    ],
    GREYS.grey,
  );
  polygon(
    parent,
    [
      [104, 24],
      [122, 22],
      [114, 72],
      [108, 50],
    ],
    GREYS.dark,
  );
  polygon(
    parent,
    [
      [84, 74],
      [104, 24],
      [96, 30],
    ],
    GREYS.pale,
  );
  facetted(parent, 6, 150, 70, 19, { lit: GREYS.white, mid: WHITE.mid, shaded: GREYS.pale });
  polygon(
    parent,
    [
      [166, 70],
      [192, 76],
      [166, 76],
    ],
    HAZARD_ORANGE.lit,
  );
  polygon(
    parent,
    [
      [166, 76],
      [192, 76],
      [168, 82],
    ],
    HAZARD_ORANGE.shaded,
  );
  polygon(parent, ngon(6, 156, 64, 5), GULL_EYE);
  polygon(parent, ngon(4, 157, 64, 2), INK);
  line(parent, [146, 57], [162, 60], INK, 2.5);
  line(parent, [92, 110], [88, 128], HAZARD_ORANGE.lit, 3);
  line(parent, [112, 108], [110, 128], HAZARD_ORANGE.lit, 3);
};

/** A snail with a rocket strapped to its faceted shell, regretting it. */
const boost: Drawing = (parent) => {
  shadow(parent, 100, 146, 70, 5);
  polygon(
    parent,
    [
      [30, 130],
      [150, 130],
      [176, 112],
      [160, 102],
      [60, 110],
      [34, 120],
    ],
    GREYS.grey,
  );
  polygon(
    parent,
    [
      [60, 110],
      [160, 102],
      [168, 108],
      [64, 115],
    ],
    GREYS.pale,
  );
  polygon(
    parent,
    [
      [32, 126],
      [154, 126],
      [150, 132],
      [30, 132],
    ],
    GREYS.dark,
  );
  line(parent, [158, 86], [150, 58], GREYS.dark, 4);
  line(parent, [168, 86], [174, 58], GREYS.grey, 4);
  facetted(parent, 6, 162, 96, 16, { lit: GREYS.pale, mid: GREYS.grey, shaded: GREYS.dark });
  polygon(parent, ngon(6, 150, 54, 8), GREYS.white);
  polygon(parent, ngon(6, 174, 54, 8), WHITE.mid);
  polygon(parent, ngon(4, 153, 55, 3.4), INK);
  polygon(parent, ngon(4, 177, 55, 3.4), INK);
  stroke(
    parent,
    [
      [164, 104],
      [170, 107],
      [176, 104],
    ],
    INK,
    2.5,
  );
  facetted(parent, 7, 90, 84, 36, HAZARD_ORANGE);
  facetted(parent, 6, 94, 82, 19, tonesOf(HAZARD_ORANGE.lit));
  polygon(parent, ngon(5, 97, 81, 7), HAZARD_ORANGE.shaded);
  polygon(
    parent,
    [
      [80, 56],
      [88, 56],
      [92, 100],
      [84, 100],
    ],
    ROCKET_STRAP,
  );
  polygon(
    parent,
    [
      [48, 38],
      [18, 46],
      [48, 54],
    ],
    ROCKET_FLAME,
  );
  polygon(
    parent,
    [
      [48, 41],
      [30, 46],
      [48, 51],
    ],
    HAZARD_ORANGE.lit,
  );
  polygon(
    parent,
    [
      [52, 32],
      [118, 32],
      [122, 40],
      [48, 40],
    ],
    ROCKET.lit,
  );
  polygon(
    parent,
    [
      [48, 40],
      [122, 40],
      [122, 50],
      [48, 50],
    ],
    ROCKET.mid,
  );
  polygon(
    parent,
    [
      [48, 50],
      [122, 50],
      [118, 58],
      [52, 58],
    ],
    ROCKET.shaded,
  );
  polygon(
    parent,
    [
      [122, 32],
      [142, 45],
      [122, 45],
    ],
    ROCKET.lit,
  );
  polygon(
    parent,
    [
      [122, 45],
      [142, 45],
      [122, 58],
    ],
    ROCKET.shaded,
  );
  polygon(
    parent,
    [
      [50, 32],
      [38, 20],
      [62, 32],
    ],
    GREYS.grey,
  );
  polygon(
    parent,
    [
      [50, 58],
      [38, 70],
      [62, 58],
    ],
    GREYS.dark,
  );
  polygon(parent, ngon(6, 100, 45, 6.5), GREYS.white);
  polygon(parent, ngon(6, 101, 46, 3.5), ROCKET_WINDOW);
};

const DRAWINGS: Readonly<Record<Item, Drawing>> = { boost, hazard, homing };

/**
 * The item's character as an inline SVG, at whatever size the slot gives
 * it. Decorative: the slot keeps the item's name as its text, so the
 * character is hidden from assistive technology.
 */
export function itemSprite(item: Item): SVGSVGElement {
  const sprite = svg("0 0 200 160");
  sprite.dataset.sprite = item;
  DRAWINGS[item](sprite);
  return sprite;
}
