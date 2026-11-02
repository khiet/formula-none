import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize } from "./bridge";
import { kartReadout } from "./kart-readout-fixture";
import type { Particle, Particles } from "./particles";
import { createWindLines, createWindParticles, windFade, windRate } from "./wind-lines";

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

/** Lines thrown over `frames` frames of the resting kart with `overrides`, a wake charging over a second. */
function thrown(
  overrides: Partial<KartReadout>,
  frames: number,
  reducedMotion = false,
): Particle[] {
  const particles = createWindParticles();
  const lines = createWindLines(
    particles,
    SIZE,
    { reducedMotion, chargeSeconds: 1 },
    fixedRandom(),
  );
  for (let frame = 0; frame < frames; frame++) {
    lines.emit(kartReadout(overrides), FRAME_S);
  }
  return particlesOf(particles);
}

describe("wind lines", () => {
  it("throws nothing off a boost", () => {
    expect(thrown({ boostS: null }, 30)).toEqual([]);
  });

  it("throws lines ahead of the kart and carries them back past it, outside its body", () => {
    const lines = thrown({ boostS: 1 }, 10);
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
    const lines = createWindLines(particles, SIZE, { chargeSeconds: 1 }, fixedRandom());
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
    expect(thrown({ boostS: 1.5 }, 30).length).toBeGreaterThan(thrown({ boostS: 0.2 }, 30).length);
  });

  it("thins the lines under reduced motion", () => {
    const full = thrown({ boostS: 1 }, 30).length;
    const thinned = thrown({ boostS: 1 }, 30, true).length;
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

describe("wind lines in a slipstream", () => {
  const charging = (s: number): Partial<KartReadout> => ({ slipstream: { charging: { s } } });
  const towed = (leftS: number): Partial<KartReadout> => ({ slipstream: { tow: { leftS } } });

  it("throws lines at a rate climbing from nothing with the charge to the least boost rate", () => {
    expect(windRate(kartReadout(charging(0)), 1)).toBe(0);
    expect(windRate(kartReadout(charging(0.5)), 1)).toBeCloseTo(20);
    expect(windRate(kartReadout(charging(1)), 1)).toBe(40);
    expect(windRate(kartReadout(charging(0.5)), 2)).toBeCloseTo(10);
    expect(thrown(charging(0.9), 30).length).toBeGreaterThan(thrown(charging(0.3), 30).length);
    expect(thrown(charging(0.05), 30).length).toBeLessThan(thrown(charging(0.9), 30).length / 4);
  });

  it("still throws a thin charge's lines under reduced motion, which thins them under ten a second", () => {
    const thinned = thrown(charging(0.5), 60, true).length;
    expect(thinned).toBeGreaterThan(0);
    expect(thinned).toBeLessThan(thrown(charging(0.5), 60).length);
  });

  it("throws a tow's lines at the boost's full rate, and a boost under way wins over the tow", () => {
    expect(windRate(kartReadout(towed(1)), 1)).toBe(90);
    expect(windRate(kartReadout(towed(0.1)), 1)).toBe(90);
    expect(windRate(kartReadout({ boostS: 0.2, slipstream: { tow: { leftS: 1 } } }), 1)).toBe(
      windRate(kartReadout({ boostS: 0.2 }), 1),
    );
    expect(windRate(kartReadout({ boostS: 1.5, slipstream: { charging: { s: 0.1 } } }), 1)).toBe(
      90,
    );
    expect(windRate(kartReadout(), 1)).toBe(0);
  });

  it("shows the lines fully through a charge and fades them over the tow's last 0.35 s", () => {
    expect(windFade(null, { charging: { s: 0.1 } })).toBe(1);
    expect(windFade(null, { tow: { leftS: 1 } })).toBe(1);
    expect(windFade(null, { tow: { leftS: 0.175 } })).toBeCloseTo(0.5);
    expect(windFade(null, { tow: { leftS: 0 } })).toBe(0);
    // A boost under way sets the fade, whatever the tow has left.
    expect(windFade(0.0875, { tow: { leftS: 1 } })).toBeCloseTo(0.25);
    expect(windFade(null, null)).toBe(0);
  });
});
