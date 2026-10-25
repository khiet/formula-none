import type { Item } from "./generated/protocol";

/**
 * The item slot's characters (ADR 0024): one 16x16 sprite per item the
 * protocol knows, authored as string grids so a change is a readable diff.
 * Original work, CC0 like the rest of the art (assets/LICENSES.md).
 */

/** The side of every sprite's grid, in sprite pixels. */
export const SPRITE_SIZE = 16;

/** The most colours a sprite may use, the outline included, so each stays a 16-bit sprite and not a picture. */
export const MAX_SPRITE_COLOURS = 8;

/** The panel's outline brown: every sprite's outline, so the characters read as part of the slot's frame. */
export const SPRITE_OUTLINE = "#3a1c0a";

/**
 * One character per colour. `.` is transparent. The colours are the panel's
 * own (ADR 0023), the View's hazard cone, livery red and a grey: nothing
 * that reads only against the slot's gold. The cone's orange and livery red
 * are copied from the View and the livery table, so a change there is a
 * change here too.
 */
export const SPRITE_PALETTE: Readonly<Record<string, string>> = {
  k: SPRITE_OUTLINE,
  y: "#fde97a", // the panel's gold
  w: "#ffffff", // the panel's label white
  o: "#ff5522", // the View's hazard cone
  r: "#d62828", // kart 0's livery red
  s: "#9aa0a8", // grey
};

/**
 * The grids, `SPRITE_SIZE` rows of `SPRITE_SIZE` characters from
 * `SPRITE_PALETTE` or `.`. What survives the size is the face and the one
 * prop: eyes and a rocket, eyes and a stripe, an eye and a beak.
 */
export const ITEM_SPRITES: Readonly<Record<Item, readonly string[]>> = {
  // A snail with a rocket strapped to its shell, regretting it.
  boost: [
    "................",
    "...kkkkkkkk.....",
    ".okrrrwrrrrk....",
    "ookrrrwrrrryk...",
    ".okrrrrrrrrk.k.k",
    "..kkkkkkkkkk.k.k",
    ".koooook.kkkkkkk",
    "kookkkookssssssk",
    "kokoookokwwswwsk",
    "kokokokokwkswksk",
    "kokoookoksskkssk",
    "kookkkkokssssssk",
    "kkkkkkkkkssssssk",
    "kssssssssssssssk",
    ".kkkkkkkkkkkkkk.",
    "................",
  ],
  // A traffic cone that hates you: V brows and a flat mouth.
  hazard: [
    "................",
    ".......kk.......",
    "......kook......",
    "......kook......",
    ".....kkookk.....",
    ".....kokkok.....",
    "....kwwoowwk....",
    "....kwkookwk....",
    "...kowwoowwok...",
    "...kwwwwwwwwk...",
    "..kwwwwwwwwwwk..",
    "..kooookkooook..",
    ".kkkkkkkkkkkkkk.",
    ".kssssssssssssk.",
    ".kkkkkkkkkkkkkk.",
    "................",
  ],
  // A seagull with its eye locked on, wing up, beak forward.
  homing: [
    "................",
    "..kk............",
    ".kssk...........",
    ".kswwk..kkkkkk..",
    "..kwwwk.kwwwwk..",
    "...kwwwkkkkkwkk.",
    "....kwwwkkrkwook",
    ".....kwwkkkkwkk.",
    ".kkkkkkkkwwwwk..",
    "kwwwwwwwwwwwwk..",
    "kkwwwwwwwwwwwk..",
    ".kssssssssssk...",
    "..kkkkkkkkkk....",
    "................",
    "................",
    "................",
  ],
};

const SVG_NS = "http://www.w3.org/2000/svg";

/**
 * The item's sprite as an inline SVG of one rect per painted pixel, drawn
 * crisp at whatever size the slot gives it. Decorative: the slot keeps the
 * item's name as its text, so the sprite is hidden from assistive technology.
 */
export function itemSprite(item: Item): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${SPRITE_SIZE} ${SPRITE_SIZE}`);
  svg.setAttribute("shape-rendering", "crispEdges");
  svg.setAttribute("aria-hidden", "true");
  svg.dataset.sprite = item;
  ITEM_SPRITES[item].forEach((row, y) => {
    [...row].forEach((key, x) => {
      const colour = SPRITE_PALETTE[key];
      if (colour === undefined) return;
      const pixel = document.createElementNS(SVG_NS, "rect");
      pixel.setAttribute("x", String(x));
      pixel.setAttribute("y", String(y));
      pixel.setAttribute("width", "1");
      pixel.setAttribute("height", "1");
      pixel.setAttribute("fill", colour);
      svg.append(pixel);
    });
  });
  return svg;
}
