import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { pitLanePath, projectCentreline } from "./course-map";

// A rectangle twice as wide (x) as it is deep (z), starting on its left
// edge and heading up the map (-z) the way a grid faces the first corner.
const rectangle: readonly (readonly [number, number, number])[] = [
  [0, 0, 50],
  [0, 0, 0],
  [100, 0, 0],
  [100, 0, 50],
];
const box = { width: 200, height: 200, margin: 10 } as const;
const tickHalf = 5;
const deckHalf = 20;

/**
 * A figure eight in a 100 m square: the lap runs corner to corner from
 * (0, 0) to (100, 100) at `first` metres up, then back across from
 * (100, 0) to (0, 100) at `second`, the two legs crossing at the middle.
 */
function figureEight(first: number, second: number): (readonly [number, number, number])[] {
  return [
    [0, first, 0],
    [100, first, 100],
    [100, second, 0],
    [0, second, 100],
  ];
}

/** The points an open SVG path of `M` and `L` commands runs through. */
function pointsOf(path: string): [number, number][] {
  const numbers = path.replace(/[ML]/g, " ").trim().split(/\s+/).map(Number);
  return Array.from({ length: numbers.length / 2 }, (_, i) => [
    numbers[2 * i] as number,
    numbers[2 * i + 1] as number,
  ]);
}

describe("projecting the centreline onto the course map", () => {
  it("fits the outline to the box with the margin, keeping the aspect ratio", () => {
    const course = projectCentreline(rectangle, box, tickHalf, deckHalf);
    // The wide side fills the box inside the margin; the deep side is
    // scaled the same and centred, so the rectangle stays twice as wide.
    expect(course.toMap(0, 0)).toEqual([10, 55]);
    expect(course.toMap(100, 50)).toEqual([190, 145]);
    expect(course.toMap(50, 25)).toEqual([100, 100]);
  });

  it("looks down on the x/z plane without mirroring: +x is right and +z is down", () => {
    const course = projectCentreline(rectangle, box, tickHalf, deckHalf);
    const [leftU, topV] = course.toMap(0, 0);
    const [rightU, bottomV] = course.toMap(100, 50);
    expect(rightU).toBeGreaterThan(leftU);
    expect(bottomV).toBeGreaterThan(topV);
  });

  it("ignores height", () => {
    const flat = projectCentreline(rectangle, box, tickHalf, deckHalf);
    const hilly = projectCentreline(
      rectangle.map(([x, , z], i) => [x, i * 7, z] as const),
      box,
      tickHalf,
      deckHalf,
    );
    expect(hilly.path).toBe(flat.path);
    expect(hilly.toMap(30, 10)).toEqual(flat.toMap(30, 10));
  });

  it("draws the outline as one closed path through every point", () => {
    const course = projectCentreline(rectangle, box, tickHalf, deckHalf);
    expect(course.path).toBe("M 10 145 L 10 55 L 190 55 L 190 145 Z");
  });

  it("marks the start across the track at the first point", () => {
    const course = projectCentreline(rectangle, box, tickHalf, deckHalf);
    // Heading up the map from the first point, the tick lies flat across
    // it, reaching the half-length either side of the point.
    const [x1, y1, x2, y2] = course.startTick;
    expect(y1).toBe(145);
    expect(y2).toBe(145);
    expect(x1).toBeCloseTo(5);
    expect(x2).toBeCloseTo(15);
  });

  it("draws no deck over a course that never crosses itself", () => {
    expect(projectCentreline(rectangle, box, tickHalf, deckHalf).decks).toEqual([]);
  });

  it("draws a deck along the higher leg where the course crosses itself, the half-length either side of the crossing", () => {
    // The square fills the box: 100 m is 180 map units, so the crossing is at (100, 100).
    for (const [first, second, over] of [
      [5, 0, [1, 1]],
      [0, 5, [-1, 1]],
    ] as const) {
      const course = projectCentreline(figureEight(first, second), box, tickHalf, deckHalf);
      expect(course.decks).toHaveLength(1);
      const points = pointsOf(course.decks[0] ?? "");
      const start = points[0] as [number, number];
      const end = points[points.length - 1] as [number, number];
      const half = deckHalf / Math.SQRT2;
      for (const [point, side] of [
        [start, -1],
        [end, 1],
      ] as const) {
        expect(point[0]).toBeCloseTo(100 + side * half * over[0]);
        expect(point[1]).toBeCloseTo(100 + side * half * over[1]);
      }
    }
  });

  it("draws Bell Deer with one deck, where the back straight bridges the run to the hairpin", () => {
    const sidecar = JSON.parse(
      readFileSync(
        new URL("../../../assets/tracks/bell-deer/bell-deer.track.json", import.meta.url),
        "utf8",
      ),
    ) as { centreline: [number, number, number][] };
    const course = projectCentreline(sidecar.centreline, box, tickHalf, deckHalf);
    expect(course.decks).toHaveLength(1);
    // The deck's middle is the back straight, about 1021 m into the lap, 5 m up.
    const [deckX, , deckZ] = sidecar.centreline[1021] as [number, number, number];
    const points = pointsOf(course.decks[0] ?? "");
    const middle = points[Math.floor(points.length / 2)] as [number, number];
    const [u, v] = course.toMap(deckX, deckZ);
    expect(Math.hypot(middle[0] - u, middle[1] - v)).toBeLessThan(1);
  });
});

describe("the pit lane on the course map", () => {
  // Along the bottom of a wide rectangle, the lane 16 m to one side of the
  // straight and joining it at both ends, as a lane is near the road's scale.
  const straight: readonly (readonly [number, number, number])[] = [
    [0, 0, 0],
    [100, 0, 0],
    [200, 0, 0],
    [200, 0, 100],
    [0, 0, 100],
  ];
  const lane: readonly (readonly [number, number, number])[] = [
    [50, 0, 4],
    [80, 0, 16],
    [120, 0, 16],
    [150, 0, 4],
  ];
  const projection = projectCentreline(straight, box, tickHalf, deckHalf);
  const roadAt = (x: number) => projection.toMap(x, 0)[1];

  it("stands the lane clear of the road beside the straight and joins it at the ends", () => {
    const beside = 8;
    const points = pointsOf(pitLanePath(projection, straight, lane, beside));
    expect(points).toHaveLength(lane.length);
    const [, middleV] = points[1] as [number, number];
    expect(middleV - roadAt(80)).toBeCloseTo(beside, 5);
    const [, endV] = points[0] as [number, number];
    expect(endV - roadAt(50)).toBeCloseTo(beside / 4, 5);
  });
});
