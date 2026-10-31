// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { courseStats, createCoursePreview } from "./course-preview";
import { COURSES, type WorldPoint } from "./track";

/**
 * A stadium sampled every metre or so, as the exporter samples a layout:
 * two 100 m straights joined by half circles of radius 20, a 326 m lap.
 */
function stadium(): WorldPoint[] {
  const points: WorldPoint[] = [];
  const straight = (x0: number, z: number, length: number, direction: 1 | -1) => {
    for (let i = 0; i < length; i += 1) points.push([x0 + direction * i, 0, z]);
  };
  const half = (cx: number, cz: number, radius: number, from: number) => {
    const steps = Math.round(Math.PI * radius);
    for (let i = 0; i < steps; i += 1) {
      const a = from + (Math.PI * i) / steps;
      points.push([cx + radius * Math.cos(a), 0, cz + radius * Math.sin(a)]);
    }
  };
  straight(0, 0, 100, 1);
  half(100, 20, 20, -Math.PI / 2);
  straight(100, 40, 100, -1);
  half(0, 20, 20, Math.PI / 2);
  return points;
}

describe("a course's numbers from its centreline", () => {
  it("measures the lap, the tightest bend and the longest straight", () => {
    const stats = courseStats(stadium());
    expect(stats.lapM).toBeCloseTo(200 + 2 * Math.PI * 20, -1);
    expect(stats.tightestM).toBeCloseTo(20, 0);
    expect(stats.straightM).toBeCloseTo(100, -1);
  });
});

describe("the course preview", () => {
  it("shows the course's name, its circuit, its blurb and its four numbers", () => {
    const preview = createCoursePreview({ laps: 3, still: true });
    const course = COURSES.find((entry) => entry.track === "goldstone");
    if (course === undefined) throw new Error("no Goldstone");
    preview.show(course, stadium());
    const text = preview.element.textContent ?? "";
    expect(text).toContain("Goldstone");
    expect(text).toContain("After Silverstone");
    expect(text).toContain(course.blurb);
    expect(text).toContain("326 m");
    expect(text).toContain("3Laps");
    expect(text).toContain("20 m");
    expect(text).toContain("100 m");
    preview.dispose();
  });
});
