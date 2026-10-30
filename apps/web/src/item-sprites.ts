import type { Item } from "./generated/protocol";
import { hexCss } from "./livery";
import { group, line, ngon, type Point, polygon, rectangle, SVG_NS, svg } from "./low-poly-svg";
import {
  CARBON,
  CHAMPAGNE_BOTTLE,
  CORK,
  DRONE_EYE,
  GREYS,
  HAZARD_ORANGE,
  INK,
  POSTER_RED,
  ROCKET,
  ROCKET_FLAME,
  ROCKET_STRAP,
  tonesOf,
  VALUE_WHITE,
  WHITE,
} from "./palette";

/**
 * The item slot's characters (ADR 0037): each item drawn flat from the
 * View's mesh for it, in polygons lifted from the style guide's drawings
 * (docs/style-guide.html). The champagne bottle has no mesh, since a
 * boost never shows on the road. Original work, CC0 like the rest of the art
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

/**
 * The podium bottle, shaken: tilted up to the right, its cork flying off the
 * neck on a jet of foam. No face; the cork and the foam carry it.
 */
const boost: Drawing = (parent) => {
  shadow(parent, 96, 146, 48, 5);
  // Drawn along the bottle's own axis, base to cork, then tilted.
  const bottle = group(parent);
  bottle.setAttribute("transform", "translate(88 92) rotate(-38) scale(0.84)");
  for (const [x, y, r] of [
    [64, 0, 10],
    [76, -11, 9],
    [78, 11, 8],
    [92, -3, 11],
    [104, -16, 7],
    [106, 12, 8],
    [118, -2, 6],
  ] as const) {
    polygon(bottle, ngon(7, x, y, r), WHITE.mid);
    polygon(bottle, ngon(7, x - 2, y - 2, r * 0.6), VALUE_WHITE);
  }
  polygon(
    bottle,
    [
      [128, -8],
      [144, -10],
      [144, 10],
      [128, 8],
    ],
    CORK.mid,
  );
  polygon(
    bottle,
    [
      [128, -8],
      [144, -10],
      [144, -3],
      [128, -2],
    ],
    CORK.lit,
  );
  polygon(bottle, rectangle(-62, -22, 10, -8), CHAMPAGNE_BOTTLE.lit);
  polygon(bottle, rectangle(-62, -8, 10, 8), CHAMPAGNE_BOTTLE.mid);
  polygon(bottle, rectangle(-62, 8, 10, 22), CHAMPAGNE_BOTTLE.shaded);
  polygon(
    bottle,
    [
      [10, -22],
      [32, -8],
      [32, 0],
      [10, 0],
    ],
    CHAMPAGNE_BOTTLE.lit,
  );
  polygon(
    bottle,
    [
      [10, 0],
      [32, 0],
      [32, 8],
      [10, 22],
    ],
    CHAMPAGNE_BOTTLE.shaded,
  );
  const foil = tonesOf(ROCKET_FLAME);
  polygon(bottle, rectangle(30, -8, 58, 0), foil.lit);
  polygon(bottle, rectangle(30, 0, 58, 8), foil.shaded);
  polygon(bottle, rectangle(-46, -16, -12, 16), WHITE.mid);
  polygon(bottle, rectangle(-46, -4, -12, 4), ROCKET.mid);
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
