import type { Item } from "./generated/protocol";
import { hexCss } from "./livery";
import { facetted, line, ngon, type Point, polygon, rectangle, SVG_NS, svg } from "./low-poly-svg";
import {
  CARBON,
  DRONE_EYE,
  GREYS,
  HAZARD_ORANGE,
  INK,
  POSTER_RED,
  ROCKET,
  ROCKET_FLAME,
  ROCKET_STRAP,
  ROCKET_WINDOW,
  tonesOf,
  VALUE_WHITE,
  WHITE,
} from "./palette";

/**
 * The item slot's characters (ADR 0037): each item drawn flat from the
 * View's mesh for it, in polygons lifted from the style guide's drawings
 * (docs/style-guide.html). The snail has no mesh, since a boost
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

/** A rotor's blur seen edge on: a flat white ellipse under the blade. */
function blur(parent: Element, cx: number, cy: number, rx: number, ry: number): void {
  const element = document.createElementNS(SVG_NS, "ellipse");
  for (const [name, value] of Object.entries({ cx, cy, rx, ry })) {
    element.setAttribute(name, String(value));
  }
  element.setAttribute("fill", hexCss(VALUE_WHITE));
  element.setAttribute("fill-opacity", "0.55");
  parent.append(element);
}

/**
 * A camera quad drone side on, nose right: carbon arms, orange motors
 * under blurred rotors, a red canopy, the camera pod's red lens locked on
 * and the antenna's tally light.
 */
const homing: Drawing = (parent) => {
  shadow(parent, 100, 146, 56, 5);
  line(parent, [66, 80], [48, 46], ROCKET_STRAP, 3);
  polygon(parent, ngon(6, 48, 45, 4), DRONE_EYE);
  polygon(parent, rectangle(24, 84, 176, 94), CARBON.lit);
  polygon(parent, rectangle(24, 84, 176, 87), GREYS.deep);
  for (const x of [38, 162]) {
    polygon(parent, rectangle(x - 9, 66, x + 9, 88), HAZARD_ORANGE.mid);
    polygon(parent, rectangle(x - 9, 66, x + 9, 71), HAZARD_ORANGE.lit);
    blur(parent, x, 62, 34, 5);
    line(parent, [x - 30, 62], [x + 30, 62], CARBON.mid, 3);
  }
  polygon(
    parent,
    [
      [60, 78],
      [140, 78],
      [148, 100],
      [52, 100],
    ],
    CARBON.mid,
  );
  polygon(
    parent,
    [
      [70, 78],
      [84, 56],
      [118, 54],
      [134, 78],
    ],
    POSTER_RED.mid,
  );
  polygon(
    parent,
    [
      [70, 78],
      [84, 56],
      [100, 55],
      [92, 78],
    ],
    POSTER_RED.lit,
  );
  polygon(parent, rectangle(114, 98, 154, 126), CARBON.shaded);
  polygon(parent, ngon(8, 154, 112, 13), WHITE.mid);
  polygon(parent, ngon(8, 154, 112, 9), DRONE_EYE);
  polygon(parent, ngon(6, 155, 111, 3), INK);
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
