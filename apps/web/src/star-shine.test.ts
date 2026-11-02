import { describe, expect, it } from "vitest";
import { kartReadout } from "./kart-readout-fixture";
import { createParticles, type Particle } from "./particles";
import {
  createStarSparkles,
  HIT_BURST_COUNT,
  mixColour,
  STAR_LOOP_S,
  STAR_PULL,
  STAR_RAINBOW,
  STAR_STILL_PULL,
  STAR_STILL_SHINE,
  STAR_TILES,
  starColour,
  starPull,
  starShine,
  starTileColour,
} from "./star-shine";

const SIZE = { width: 1.2, height: 0.6, length: 1.8 };

describe("the star's colour", () => {
  it("lands on each stop in turn through one loop, and loops round", () => {
    const step = STAR_LOOP_S / STAR_RAINBOW.length;
    STAR_RAINBOW.forEach((stop, index) => {
      expect(starColour(index * step)).toBe(stop);
      expect(starColour(index * step + STAR_LOOP_S)).toBe(stop);
      expect(starColour(index * step + 4 * STAR_LOOP_S)).toBe(stop);
    });
  });

  it("blends between neighbouring stops rather than stepping", () => {
    const step = STAR_LOOP_S / STAR_RAINBOW.length;
    const channels = (colour: number) => [16, 8, 0].map((shift) => (colour >> shift) & 0xff);
    const between = (colour: number, a: number, b: number) =>
      channels(colour).every((channel, index) => {
        const [from, to] = [channels(a)[index] ?? 0, channels(b)[index] ?? 0];
        return channel >= Math.min(from, to) && channel <= Math.max(from, to);
      });
    const [red, orange] = STAR_RAINBOW;
    if (red === undefined || orange === undefined) throw new Error("six stops");
    const halfway = starColour(step / 2);
    expect(halfway).not.toBe(red);
    expect(halfway).not.toBe(orange);
    expect(between(halfway, red, orange)).toBe(true);
    // The last stop blends back into the first.
    const violet = STAR_RAINBOW[STAR_RAINBOW.length - 1] ?? 0;
    expect(between(starColour(STAR_LOOP_S - step / 2), violet, red)).toBe(true);
    expect(mixColour(0x000000, 0xffffff, 0.5)).toBe(0x808080);
  });

  it("is the same colour on every client for the same seconds left", () => {
    expect(starColour(4.37)).toBe(starColour(4.37));
    expect(starColour(4.37)).not.toBe(starColour(4.2));
  });

  it("steps each of the ball's tiles one stop along from the one before", () => {
    const step = STAR_LOOP_S / STAR_TILES;
    for (let tile = 1; tile < STAR_TILES; tile++) {
      expect(starTileColour(2, tile)).toBe(starColour(2 - tile * step));
    }
    expect(starTileColour(2, 0)).toBe(starColour(2));
  });
});

describe("the star's pull", () => {
  it("holds the full pull through the run and fades it to nothing over the last second", () => {
    expect(starPull(6)).toBe(STAR_PULL);
    expect(starPull(1)).toBe(STAR_PULL);
    expect(starPull(0.5)).toBeCloseTo(STAR_PULL / 2, 6);
    expect(starPull(0)).toBe(0);
  });

  it("shines nothing off a run, the loop on a run, and a steady white under reduced motion", () => {
    expect(starShine(null, false)).toBeNull();
    expect(starShine(null, true)).toBeNull();
    expect(starShine(3.1, false)).toEqual({ colour: starColour(3.1), pull: STAR_PULL });
    expect(starShine(3.1, true)).toEqual({ colour: STAR_STILL_SHINE, pull: STAR_STILL_PULL });
    expect(starShine(0.5, true)?.pull).toBe(STAR_STILL_PULL);
  });
});

describe("the star's sparkles", () => {
  const live = (particles: ReturnType<typeof createParticles>) => {
    const seen: Particle[] = [];
    particles.forEach((particle) => {
      seen.push({ ...particle });
    });
    return seen;
  };

  it("throws a few sparkles a second off the kart, white and cooling to the shine, left behind it", () => {
    const particles = createParticles({ capacity: 64 });
    const sparkles = createStarSparkles(particles, SIZE, () => 0.5);
    const kart = kartReadout({ position: [10, 0.3, -20] });
    for (let frame = 0; frame < 60; frame++) sparkles.emit(kart, 0xff4d4d, 1 / 60);
    const thrown = live(particles);
    expect(thrown.length).toBeGreaterThanOrEqual(6);
    expect(thrown.length).toBeLessThanOrEqual(10);
    for (const sparkle of thrown) {
      expect(sparkle.colour).toBe(0xffffff);
      expect(Math.abs(sparkle.position[0] - 10)).toBeLessThanOrEqual(SIZE.width);
      expect(Math.abs(sparkle.position[2] + 20)).toBeLessThanOrEqual(SIZE.length);
    }
  });

  it("bursts a ring of sparks out from a hit", () => {
    const particles = createParticles({ capacity: 64 });
    const sparkles = createStarSparkles(particles, SIZE, () => 0.5);
    sparkles.burst([1, 0.3, 2], 0x3ddc84);
    const thrown = live(particles);
    expect(thrown).toHaveLength(HIT_BURST_COUNT);
    for (const spark of thrown) {
      expect(spark.position[0]).toBeCloseTo(1, 5);
      expect(spark.position[1]).toBeCloseTo(0.3, 5);
      expect(spark.position[2]).toBeCloseTo(2, 5);
    }
  });
});
