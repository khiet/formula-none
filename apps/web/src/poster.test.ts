// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import side from "./generated/kart/side.json";
import { FACIAL_HAIRS, HAIRS, hexCss, LIVERIES, livery } from "./livery";
import { KART_PAINT } from "./palette";
import { kartSide } from "./poster";
import { WHEELS } from "./wheel-spin";

/** Every point in `d`, as `[x, y]`. */
const points = (d: string): [number, number][] =>
  [...d.matchAll(/[ML](-?[\d.]+) (-?[\d.]+)/g)].map(([, x = "", y = ""]) => [Number(x), Number(y)]);

describe("the generated poster kart", () => {
  it("carries a node for every hair style, every facial hair and the earrings, so each driver can be dressed", () => {
    const nodes = new Set(side.nodes);
    for (const hair of HAIRS) expect(nodes).toContain(`hair-${hair}`);
    for (const facialHair of FACIAL_HAIRS.filter((kind) => kind !== "none")) {
      expect(nodes).toContain(`facial-hair-${facialHair}`);
    }
    expect(nodes).toContain("earrings");
    // Every hair style and the earrings show side on; a stubble may lie wholly on the chin's forward flat.
    const drawn = new Set(side.paths.map(({ node }) => node));
    for (const hair of HAIRS) expect(drawn).toContain(`hair-${hair}`);
    expect(drawn).toContain("earrings");
  });

  it("places all four wheels, each with its axle inside the picture", () => {
    expect(Object.keys(side.wheels).sort()).toEqual([...WHEELS].sort());
    for (const [x, y] of Object.values(side.wheels)) {
      expect(x).toBeGreaterThan(0);
      expect(x).toBeLessThan(520);
      expect(y).toBeGreaterThan(0);
      expect(y).toBeLessThan(380);
    }
  });

  it("fits the whole projection, tallest hair included, inside 520 x 380 with its wheels on the ground line", () => {
    const all = side.paths.flatMap(({ d }) => points(d));
    expect(all.length).toBeGreaterThan(100);
    expect(Math.min(...all.map(([x]) => x))).toBeGreaterThanOrEqual(0);
    expect(Math.max(...all.map(([x]) => x))).toBeLessThanOrEqual(520);
    expect(Math.min(...all.map(([, y]) => y))).toBeGreaterThanOrEqual(0);
    expect(Math.max(...all.map(([, y]) => y))).toBe(350);
  });
});

describe("kartSide", () => {
  it.each(Array.from({ length: LIVERIES }, (_, slot) => slot))(
    "draws slot %i in at most 120 paths, each wheel in one group about its axle",
    (slot) => {
      const kart = kartSide(livery(slot));
      expect(kart.querySelectorAll("path").length).toBeLessThanOrEqual(120);
      const wheels = [...kart.querySelectorAll<SVGGElement>(".wheel")];
      expect(wheels.map((wheel) => wheel.dataset.node).sort()).toEqual([...WHEELS].sort());
      for (const wheel of wheels) {
        const [x, y] = side.wheels[wheel.dataset.node as keyof typeof side.wheels];
        expect(wheel.style.transformOrigin).toBe(`${x}px ${y}px`);
      }
    },
  );

  it("shows only the parts the driver wears", () => {
    const rosso = kartSide(livery(0));
    const nodes = (kart: SVGSVGElement) =>
      new Set([...kart.querySelectorAll<SVGGElement>("[data-node]")].map((g) => g.dataset.node));
    expect(nodes(rosso)).toContain("hair-braids-tied-back");
    expect(nodes(rosso)).toContain("facial-hair-short-beard");
    expect(nodes(rosso)).toContain("earrings");
    expect(nodes(rosso)).not.toContain("hair-short");
    const argento = kartSide(livery(2));
    expect(nodes(argento)).toContain("hair-swept-side-part");
    expect([...nodes(argento)].filter((node) => node?.startsWith("facial-hair-"))).toEqual([]);
    expect(nodes(argento)).not.toContain("earrings");
  });

  it("paints by custom property alone: the livery's, the driver's and build.py's fixed colours", () => {
    const kart = kartSide(livery(0));
    const paint = livery(0);
    for (const [name, colour] of [
      ["p", paint.primary],
      ["s", paint.secondary],
      ["a", paint.accent],
      ["sk", paint.driver.skin],
      ["hair", paint.driver.hairColour],
      ...Object.entries(KART_PAINT),
    ] as const) {
      expect(kart.style.getPropertyValue(`--${name}`)).toBe(hexCss(colour));
    }
    for (const path of kart.querySelectorAll("path")) {
      expect(path.getAttribute("fill")).toMatch(/^var\(--[a-z]+(-lt|-dk)?\)$/);
    }
  });
});
