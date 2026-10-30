import { describe, expect, it } from "vitest";
import type { Triangle, Vec3 } from "../src/glb.ts";
import { order, type Point, planes, project, visible } from "../src/project.ts";

/** A box as twelve triangles wound counter-clockwise from outside, as the glb's faces are. */
function box(node: string, material: string, centre: Vec3, size: Vec3): Triangle[] {
  const corner = (sx: number, sy: number, sz: number): Vec3 => [
    centre[0] + (sx * size[0]) / 2,
    centre[1] + (sy * size[1]) / 2,
    centre[2] + (sz * size[2]) / 2,
  ];
  const quads: [Vec3, Vec3, Vec3, Vec3][] = [
    [corner(1, -1, -1), corner(1, 1, -1), corner(1, 1, 1), corner(1, -1, 1)], // +X
    [corner(-1, -1, 1), corner(-1, 1, 1), corner(-1, 1, -1), corner(-1, -1, -1)], // -X
    [corner(-1, 1, -1), corner(-1, 1, 1), corner(1, 1, 1), corner(1, 1, -1)], // +Y
    [corner(-1, -1, 1), corner(-1, -1, -1), corner(1, -1, -1), corner(1, -1, 1)], // -Y
    [corner(-1, -1, 1), corner(1, -1, 1), corner(1, 1, 1), corner(-1, 1, 1)], // +Z
    [corner(1, -1, -1), corner(-1, -1, -1), corner(-1, 1, -1), corner(1, 1, -1)], // -Z
  ];
  return quads.flatMap(([a, b, c, d]) => [
    { node, material, corners: [a, b, c] },
    { node, material, corners: [a, c, d] },
  ]);
}

const LEVEL = { pitch: 0, light: [0, 1.3, 1] as const };

const corners = (loop: readonly Point[]) =>
  [...loop].map(([x, y]) => [Number(x.toFixed(6)), Number(y.toFixed(6))]).sort();

describe("the side projection", () => {
  it("projects a box seen level to its near side alone: the nose end to the right, the ground at the bottom", () => {
    // One metre wide, half a metre tall, two metres long, standing on the ground with its nose at z = -2.
    const faces = planes(project(box("tub", "Primary", [0, 0.25, -1], [1, 0.5, 2]), LEVEL));
    expect(faces).toHaveLength(1);
    const [side] = faces;
    expect(side?.node).toBe("tub");
    expect(side?.material).toBe("Primary");
    expect(side?.loops).toHaveLength(1);
    expect(corners(side?.loops[0] ?? [])).toEqual(
      corners([
        [0, 0],
        [2, 0],
        [2, 0.5],
        [0, 0.5],
      ]),
    );
  });

  it("culls back faces", () => {
    const towards: Triangle = {
      node: "n",
      material: "m",
      corners: [
        [0, 0, 0],
        [0, 0, -1],
        [0, 1, -1],
      ],
    };
    const away: Triangle = {
      node: "n",
      material: "m",
      corners: [
        [0, 0, 0],
        [0, 1, -1],
        [0, 0, -1],
      ],
    };
    expect(project([towards], LEVEL)).toHaveLength(1);
    expect(project([away], LEVEL)).toHaveLength(0);
  });

  it("shades by the light: a level side is mid, the top lit and the underside shaded, and the pitch shows the top as a strip", () => {
    const pitched = { pitch: (4 * Math.PI) / 180, light: [0, 1.3, 1] as const };
    const faces = planes(project(box("tub", "Primary", [0, 0.25, -1], [1, 0.5, 2]), pitched));
    expect(faces.map(({ tone }) => tone).sort()).toEqual(["lit", "mid"]);
    const top = faces.find(({ tone }) => tone === "lit");
    const side = faces.find(({ tone }) => tone === "mid");
    const highest = (loop: readonly Point[] = []) => Math.max(...loop.map(([, y]) => y));
    expect(highest(top?.loops[0])).toBeGreaterThan(highest(side?.loops[0]));
  });

  it("holds painter order: a box behind another that covers it on the picture is drawn first, whichever came first", () => {
    const far = box("far", "Secondary", [-0.5, 0.25, -1], [0.5, 0.5, 2]);
    const near = box("near", "Primary", [0.5, 0.25, -1], [0.5, 0.5, 1]);
    for (const triangles of [
      [...far, ...near],
      [...near, ...far],
    ]) {
      const drawn = order(planes(project(triangles, LEVEL))).map(({ node }) => node);
      expect(drawn).toEqual(["far", "near"]);
    }
  });

  it("holds painter order for boxes that pass through each other: the way round that shows more wins", () => {
    // A floor the tub sinks a centimetre into, seen pitched: the tub's side hides the far floor top, a band, and
    // the near floor's sliver ahead of the tub's bottom edge is given up.
    const pitched = { pitch: (4 * Math.PI) / 180, light: [0, 1.3, 1] as const };
    const floor = box("kart", "Secondary", [0, 0.1, 0], [0.8, 0.06, 1]);
    const tub = box("kart", "Primary", [0, 0.32, 0], [0.4, 0.4, 0.6]);
    const drawn = order(planes(project([...tub, ...floor], pitched)));
    const floorTop = drawn.findIndex(
      ({ material, tone }) => material === "Secondary" && tone === "lit",
    );
    const tubSide = drawn.findIndex(
      ({ material, tone }) => material === "Primary" && tone === "mid",
    );
    expect(floorTop).toBeLessThan(tubSide);
  });
});

describe("hidden faces", () => {
  const pitched = { pitch: (4 * Math.PI) / 180, light: [0, 1.3, 1] as const };

  it("drops a face wholly behind a part always drawn with it, and keeps one behind a part that may be left off", () => {
    const head = box("head", "Skin", [0, 1, 0], [0.6, 0.6, 0.6]);
    const earring = box("earrings", "Gold", [-0.25, 1, 0], [0.05, 0.05, 0.05]);
    const curl = box("hair-short-curls", "Hair", [-0.25, 1.4, 0], [0.05, 0.05, 0.05]);
    const underCurl = box("head", "Sliver", [-0.4, 1.4, 0], [0.01, 0.02, 0.02]);
    const facets = project([...head, ...earring, ...curl, ...underCurl], pitched);
    const kept = visible(facets, (node) => node === "head");
    expect(kept.some(({ node }) => node === "earrings")).toBe(false);
    expect(kept.some(({ material }) => material === "Skin")).toBe(true);
    // The curl hides the sliver, but a driver without curls would see it.
    expect(kept.some(({ material }) => material === "Sliver")).toBe(true);
  });

  it("orders parts never worn together without one hiding the other", () => {
    const head = box("head", "Skin", [0, 1, 0], [0.6, 0.6, 0.6]);
    const capA = box("hair-short", "Hair", [0.31, 1.2, 0], [0.02, 0.2, 0.6]);
    const capB = box("hair-swept-back", "Hair", [0.32, 1.2, 0], [0.02, 0.2, 0.6]);
    const together = (a: string, b: string) => !(a.startsWith("hair-") && b.startsWith("hair-"));
    const drawn = order(planes(project([...capB, ...capA, ...head], pitched)), together).map(
      ({ node }) => node,
    );
    expect([...new Set(drawn)]).toEqual(["head", "hair-short", "hair-swept-back"]);
  });
});

describe("a node drawn whole", () => {
  it("keeps its faces together even when another face lies between them in depth, ordering them among themselves", () => {
    const pitched = { pitch: (4 * Math.PI) / 180, light: [0, 1.3, 1] as const };
    // A far wheel: its wall behind a wing that reaches past it, its rim proud of the wall.
    const wall = box("wheel-front-left", "Tyre", [-0.5, 0.3, 0], [0.2, 0.6, 0.6]);
    const rim = box("wheel-front-left", "Rim", [-0.38, 0.3, 0], [0.04, 0.3, 0.3]);
    const wing = box("kart", "Secondary", [0, 0.1, 0.4], [1.2, 0.04, 0.1]);
    const drawn = order(
      planes(project([...wing, ...wall, ...rim], pitched)),
      () => true,
      (node) => node.startsWith("wheel-"),
    );
    const nodes = drawn.map(({ node }) => node);
    expect(nodes.indexOf("kart")).toBeGreaterThan(nodes.lastIndexOf("wheel-front-left"));
    const wheel = drawn.filter(({ node }) => node === "wheel-front-left");
    expect(wheel.findIndex(({ material }) => material === "Rim")).toBeGreaterThan(
      wheel.findIndex(({ material }) => material === "Tyre"),
    );
  });
});
