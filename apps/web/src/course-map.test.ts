import { describe, expect, it } from "vitest";
import { projectCourse } from "./course-map";

// A rectangle twice as wide (x) as it is deep (z), starting on its left
// edge and heading up the map (-z) the way a grid faces the first corner.
const rectangle: readonly (readonly [number, number, number])[] = [
  [0, 0, 50],
  [0, 0, 0],
  [100, 0, 0],
  [100, 0, 50],
];
const box = { width: 200, height: 200, margin: 10, startTickHalf: 5 } as const;

describe("projecting the centreline onto the course map", () => {
  it("fits the outline to the box with the margin, keeping the aspect ratio", () => {
    const course = projectCourse(rectangle, box);
    // The wide side fills the box inside the margin; the deep side is
    // scaled the same and centred, so the rectangle stays twice as wide.
    expect(course.toMap(0, 0)).toEqual([10, 55]);
    expect(course.toMap(100, 50)).toEqual([190, 145]);
    expect(course.toMap(50, 25)).toEqual([100, 100]);
  });

  it("looks down on the x/z plane without mirroring: +x is right and +z is down", () => {
    const course = projectCourse(rectangle, box);
    const [leftU, topV] = course.toMap(0, 0);
    const [rightU, bottomV] = course.toMap(100, 50);
    expect(rightU).toBeGreaterThan(leftU);
    expect(bottomV).toBeGreaterThan(topV);
  });

  it("ignores height", () => {
    const flat = projectCourse(rectangle, box);
    const hilly = projectCourse(
      rectangle.map(([x, , z], i) => [x, i * 7, z] as const),
      box,
    );
    expect(hilly.path).toBe(flat.path);
    expect(hilly.toMap(30, 10)).toEqual(flat.toMap(30, 10));
  });

  it("draws the outline as one closed path through every point", () => {
    const course = projectCourse(rectangle, box);
    expect(course.path).toBe("M 10 145 L 10 55 L 190 55 L 190 145 Z");
  });

  it("marks the start across the track at the first point", () => {
    const course = projectCourse(rectangle, box);
    // Heading up the map from the first point, the tick lies flat across
    // it, reaching the box's half-length either side of the point.
    const [x1, y1, x2, y2] = course.startTick;
    expect(y1).toBe(145);
    expect(y2).toBe(145);
    expect(x1).toBeCloseTo(5);
    expect(x2).toBeCloseTo(15);
  });
});
