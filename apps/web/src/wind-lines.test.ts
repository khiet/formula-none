import { describe, expect, it } from "vitest";
import type { KartSize } from "./bridge";
import { kartReadout } from "./kart-readout-fixture";
import type { Particle, Particles } from "./particles";
import { createWindLines, createWindParticles, windFade } from "./wind-lines";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };
const FRAME_S = 1 / 60;

function particlesOf(particles: Particles): Particle[] {
  const out: Particle[] = [];
  particles.forEach((particle) => {
    out.push({ ...particle, position: [...particle.position], tail: [...particle.tail] });
  });
  return out;
}

function fixedRandom(): () => number {
  let seed = 1;
  return () => {
    seed = (seed * 48271) % 2147483647;
    return seed / 2147483647;
  };
}

/** Lines thrown over `frames` frames of a kart holding `boostS` of boost. */
function thrown(boostS: number | null, frames: number, reducedMotion = false): Particle[] {
  const particles = createWindParticles();
  const lines = createWindLines(particles, SIZE, { reducedMotion }, fixedRandom());
  for (let frame = 0; frame < frames; frame++) {
    lines.emit(kartReadout({ boostS }), FRAME_S);
  }
  return particlesOf(particles);
}

describe("wind lines", () => {
  it("throws nothing off a boost", () => {
    expect(thrown(null, 30)).toEqual([]);
  });

  it("throws lines ahead of the kart and carries them back past it, outside its body", () => {
    const lines = thrown(1, 10);
    expect(lines.length).toBeGreaterThan(5);
    for (const line of lines) {
      // The kart faces -Z: ahead is -Z, and a streak trails toward where it came from.
      expect(line.position[2]).toBeLessThan(-SIZE.length / 2);
      expect(line.tail[2]).toBeLessThan(line.position[2]);
      const beside = Math.abs(line.position[0]) > SIZE.width / 2;
      const over = line.position[1] > SIZE.height / 2;
      expect(beside || over).toBe(true);
    }
  });

  it("carries every line past the kart within its life, even from a standstill", () => {
    const particles = createWindParticles();
    const lines = createWindLines(particles, SIZE, {}, fixedRandom());
    lines.emit(kartReadout({ boostS: 1, speed: 0 }), FRAME_S);
    for (let frame = 0; frame < 26; frame++) {
      particles.update(FRAME_S);
    }
    const left = particlesOf(particles);
    expect(left.length).toBeGreaterThan(0);
    for (const line of left) {
      // Behind the kart, which faces -Z.
      expect(line.position[2]).toBeGreaterThan(SIZE.length / 2);
    }
  });

  it("throws densest as the boost begins and fewer as it runs down", () => {
    expect(thrown(1.5, 30).length).toBeGreaterThan(thrown(0.2, 30).length);
  });

  it("thins the lines under reduced motion", () => {
    const full = thrown(1, 30).length;
    const thinned = thrown(1, 30, true).length;
    expect(thinned).toBeGreaterThan(0);
    expect(thinned).toBeLessThan(full / 2);
  });

  it("shows the lines fully until the boost's last 0.35 s, then fades them out", () => {
    expect(windFade(null)).toBe(0);
    expect(windFade(1)).toBe(1);
    expect(windFade(0.35)).toBe(1);
    expect(windFade(0.175)).toBeCloseTo(0.5);
    expect(windFade(0)).toBe(0);
  });
});
