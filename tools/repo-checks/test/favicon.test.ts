import { describe, expect, it } from "vitest";
import { readRepoFile } from "./repo";

// The struck 1: the Title logo's gold 1 with its ink outline and hard drop,
// the red strike running past both sides, on a transparent 16 px grid.
const GRID = [
  "................",
  "......kkkkk.....",
  ".....kkyyykk....",
  "....kkyyyykk....",
  "....kyyyyykk....",
  "....kkkyyykk....",
  ".kkkkkkyyykkkkk.",
  ".kRRRRRRRRRRRRkk",
  ".kRRRRRRRRRRRRkk",
  ".kkkkkkyyykkkkkk",
  "..kkkkkyyykkkkkk",
  "....kyyyyyyykk..",
  "....kyyyyyyykk..",
  "....kkkkkkkkkk..",
  ".....kkkkkkkkk..",
  "................",
];
const COLOURS: Record<string, string> = { k: "#111111", y: "#ffd23f", R: "#e8333b" };

/**
 * Paints an SVG's `<path fill d>` elements onto a size x size grid of colour
 * keys. Each subpath must be an axis-aligned rectangle drawn with M, H, V, h
 * and v; anything else, or two colours on one pixel, fails the test.
 */
function rasterise(svg: string, size: number): string[] {
  const keyOf = new Map(Object.entries(COLOURS).map(([key, hex]) => [hex, key]));
  const cells = Array.from({ length: size }, () => Array<string>(size).fill("."));
  for (const [, fill = "", d = ""] of svg.matchAll(
    /<path fill="(#[0-9a-f]{6})" d="([^"]+)"\s*\/>/g,
  )) {
    const key = keyOf.get(fill);
    expect(key, `unexpected fill ${fill}`).toBeDefined();
    for (const subpath of d.split(/z/i).filter((part) => part.trim())) {
      let x = 0;
      let y = 0;
      const xs: number[] = [];
      const ys: number[] = [];
      for (const [, op, args = ""] of subpath.matchAll(/([A-Za-z])([^A-Za-z]*)/g)) {
        const n = args
          .trim()
          .split(/[\s,]+/)
          .map(Number);
        if (op === "M") [x = 0, y = 0] = n;
        else if (op === "H") x = n[0] ?? 0;
        else if (op === "h") x += n[0] ?? 0;
        else if (op === "V") y = n[0] ?? 0;
        else if (op === "v") y += n[0] ?? 0;
        else expect.fail(`unsupported path command ${op}`);
        xs.push(x);
        ys.push(y);
      }
      for (let row = Math.min(...ys); row < Math.max(...ys); row++) {
        for (let col = Math.min(...xs); col < Math.max(...xs); col++) {
          const line = cells[row];
          expect(line?.[col], `pixel ${col},${row} painted twice`).toBe(".");
          if (line) line[col] = key ?? "?";
        }
      }
    }
  }
  return cells.map((line) => line.join(""));
}

const favicon = readRepoFile("apps/web/public/favicon.svg");
const guide = readRepoFile("docs/style-guide.html");

describe("the favicon", () => {
  it("draws the struck 1 pixel for pixel on a crisp 16 px grid", () => {
    expect(favicon).toMatch(/viewBox="0 0 16 16"/);
    expect(favicon).toMatch(/shape-rendering="crispEdges"/);
    expect(rasterise(favicon, 16)).toEqual(GRID);
  });

  it("has one title and one path per colour, on no background", () => {
    expect(favicon.match(/<title>Formula None<\/title>/g)).toHaveLength(1);
    expect(favicon).not.toMatch(/<rect/);
    const fills = [...favicon.matchAll(/<path fill="(#[0-9a-f]{6})"/g)].map(([, fill]) => fill);
    expect(fills.sort()).toEqual(Object.values(COLOURS).sort());
  });

  it("is drawn in the style guide from the same paths at 16, 32 and 64 px", () => {
    const entry = guide.match(/draw\.favicon = \(\) => (.*);$/m)?.[1] ?? "";
    expect(entry).toMatch(/\[16, 32, 64\]\.map/);
    expect(entry).toMatch(/viewBox="0 0 16 16"/);
    expect(entry).toMatch(/shape-rendering="crispEdges"/);
    expect(rasterise(entry, 16)).toEqual(GRID);
  });
});
