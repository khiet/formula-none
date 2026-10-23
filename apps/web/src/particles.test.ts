import { describe, expect, it } from "vitest";
import { createParticles, type Particle } from "./particles";

function particlesOf(particles: ReturnType<typeof createParticles>): Particle[] {
  const out: Particle[] = [];
  particles.forEach((particle) => {
    out.push({ ...particle, position: [...particle.position] });
  });
  return out;
}

describe("particles", () => {
  it("carries a particle by its velocity, under gravity, until its life runs out", () => {
    const particles = createParticles({ capacity: 4, gravity: 10 });
    particles.emit({
      position: [1, 2, 3],
      velocity: [0, 0, 4],
      colour: 0xff8800,
      lifetimeSeconds: 1,
      size: 0.1,
    });
    for (let frame = 0; frame < 50; frame++) {
      particles.update(0.01);
    }
    const [spark] = particlesOf(particles);
    expect(spark).toBeDefined();
    expect(spark?.position[0]).toBeCloseTo(1);
    expect(spark?.position[1]).toBeCloseTo(2 - 0.5 * 10 * 0.5 * 0.5, 1);
    expect(spark?.position[2]).toBeCloseTo(5);
    expect(spark?.colour).toBe(0xff8800);
    expect(spark?.size).toBeCloseTo(0.1);
    expect(spark?.life).toBeCloseTo(0.5);
    // The rest of its life, and the frame on which it is over.
    for (let frame = 0; frame <= 50; frame++) {
      particles.update(0.01);
    }
    expect(particlesOf(particles)).toEqual([]);
  });

  it("slows a particle by its drag and grows it", () => {
    const particles = createParticles({ capacity: 4, drag: 2, growth: 3 });
    particles.emit({
      position: [0, 0, 0],
      velocity: [10, 0, 0],
      colour: 0,
      lifetimeSeconds: 2,
      size: 0.5,
    });
    particles.update(0.1);
    const [puff] = particlesOf(particles);
    expect(puff?.position[0]).toBeGreaterThan(0.5);
    expect(puff?.position[0]).toBeLessThan(1);
    expect(puff?.size).toBeCloseTo(0.5 * (1 + 3 * 0.1));
    expect(puff?.position[1]).toBe(0);
  });

  it("replaces the oldest particle when it is full", () => {
    const particles = createParticles({ capacity: 2 });
    for (const colour of [1, 2, 3]) {
      particles.emit({
        position: [0, 0, 0],
        velocity: [0, 0, 0],
        colour,
        lifetimeSeconds: 1,
        size: 1,
      });
      particles.update(0.01);
    }
    expect(particlesOf(particles).map((particle) => particle.colour)).toEqual([2, 3]);
  });
});
