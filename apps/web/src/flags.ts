// The course flags the Title Scene's picker and the Room Scene's badge
// draw, as inline SVG at the real flags' proportions: Japan's at 3:2, the
// Union Flag at 2:1 and Monaco's at 5:4, which the tile crops to 3:2 from
// their centres.

import { hexCss } from "./livery";
import { SVG_NS } from "./low-poly-svg";
import { FLAG_GB_BLUE, FLAG_GB_RED, FLAG_JP_RED, FLAG_MC_RED, VALUE_WHITE } from "./palette";
import type { FlagName } from "./track";

/** Each Union Flag needs its own clip path id, since every flag on the page shares one document. */
let clipIds = 0;

function svg(viewBox: string): SVGSVGElement {
  const element = document.createElementNS(SVG_NS, "svg");
  element.setAttribute("viewBox", viewBox);
  element.setAttribute("aria-hidden", "true");
  element.dataset.flag = "";
  return element;
}

function shape(parent: Element, tag: string, attributes: Record<string, string | number>): void {
  const element = document.createElementNS(SVG_NS, tag);
  for (const [name, value] of Object.entries(attributes)) {
    element.setAttribute(name, String(value));
  }
  parent.append(element);
}

function japan(): SVGSVGElement {
  const flag = svg("0 0 900 600");
  shape(flag, "rect", { width: 900, height: 600, fill: hexCss(VALUE_WHITE) });
  shape(flag, "circle", { cx: 450, cy: 300, r: 180, fill: hexCss(FLAG_JP_RED) });
  return flag;
}

function unionFlag(): SVGSVGElement {
  const flag = svg("0 0 60 30");
  flag.setAttribute("preserveAspectRatio", "xMidYMid slice");
  const id = `union-flag-${clipIds++}`;
  // The red saltire is counterchanged: each half of it keeps to its own side of the diagonal.
  const clip = document.createElementNS(SVG_NS, "clipPath");
  clip.id = id;
  shape(clip, "path", { d: "M30,15 h30 v15 z v15 h-30 z h-30 v-15 z v-15 h30 z" });
  flag.append(clip);
  const saltire = "M0,0 L60,30 M60,0 L0,30";
  const cross = "M30,0 v30 M0,15 h60";
  shape(flag, "rect", { width: 60, height: 30, fill: hexCss(FLAG_GB_BLUE) });
  shape(flag, "path", { d: saltire, stroke: hexCss(VALUE_WHITE), "stroke-width": 6 });
  shape(flag, "path", {
    d: saltire,
    "clip-path": `url(#${id})`,
    stroke: hexCss(FLAG_GB_RED),
    "stroke-width": 4,
  });
  shape(flag, "path", { d: cross, stroke: hexCss(VALUE_WHITE), "stroke-width": 10 });
  shape(flag, "path", { d: cross, stroke: hexCss(FLAG_GB_RED), "stroke-width": 6 });
  return flag;
}

function monaco(): SVGSVGElement {
  const flag = svg("0 0 5 4");
  flag.setAttribute("preserveAspectRatio", "xMidYMid slice");
  shape(flag, "rect", { width: 5, height: 4, fill: hexCss(VALUE_WHITE) });
  shape(flag, "rect", { width: 5, height: 2, fill: hexCss(FLAG_MC_RED) });
  return flag;
}

const FLAGS: Readonly<Record<FlagName, () => SVGSVGElement>> = {
  jp: japan,
  gb: unionFlag,
  mc: monaco,
};

/** A new SVG of `name`'s flag, hidden from assistive tech: the course's name beside it says which it is. */
export function flag(name: FlagName): SVGSVGElement {
  return FLAGS[name]();
}
