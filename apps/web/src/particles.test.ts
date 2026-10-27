import { describe, expect, it } from "vitest";
import { createParticles, type Particle } from "./particles";

function particlesOf(particles: ReturnType<typeof createParticles>): Particle[] {
  const out: Particle[] = [];
  particles.forEach((particle) => {
    out.push({ ...particle, position: [...particle.position], tail: [...particle.tail] });
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

  it("cools a particle from its birth colour to its cooled colour over the cooling share of its life", () => {
    const particles = createParticles({ capacity: 4, coolShare: 0.8 });
    particles.emit({
      position: [0, 0, 0],
      velocity: [0, 0, 0],
      colour: 0xc8c8c8,
      cooledColour: 0x000000,
      lifetimeSeconds: 1,
      size: 1,
    });
    expect(particlesOf(particles)[0]?.colour).toBe(0xc8c8c8);
    particles.update(0.4);
    // Halfway through the cooling: each channel halfway.
    expect(particlesOf(particles)[0]?.colour).toBe(0x646464);
    particles.update(0.4);
    expect(particlesOf(particles)[0]?.colour).toBe(0x000000);
    particles.update(0.1);
    expect(particlesOf(particles)[0]?.colour).toBe(0x000000);
  });

  it("keeps its birth colour when it has no cooled colour", () => {
    const particles = createParticles({ capacity: 4, coolShare: 0.8 });
    particles.emit({
      position: [0, 0, 0],
      velocity: [0, 0, 0],
      colour: 0xff8800,
      lifetimeSeconds: 1,
      size: 1,
    });
    particles.update(0.5);
    expect(particlesOf(particles)[0]?.colour).toBe(0xff8800);
  });

  it("bounces a particle with restitution off the height it was born at, losing some slide", () => {
    const particles = createParticles({ capacity: 4, gravity: 10, bounceSlide: 0.5 });
    particles.emit({
      position: [0, 1, 0],
      velocity: [4, -5, 0],
      colour: 0,
      lifetimeSeconds: 5,
      size: 1,
      restitution: 0.4,
    });
    let lowest = Number.POSITIVE_INFINITY;
    let highestAfterBounce = Number.NEGATIVE_INFINITY;
    let bounced = false;
    let previousY = 1;
    for (let frame = 0; frame < 30; frame++) {
      particles.update(0.01);
      const y = particlesOf(particles)[0]?.position[1] ?? Number.NaN;
      lowest = Math.min(lowest, y);
      if (y > previousY) bounced = true;
      if (bounced) highestAfterBounce = Math.max(highestAfterBounce, y);
      previousY = y;
    }
    expect(lowest).toBeGreaterThanOrEqual(1);
    expect(bounced).toBe(true);
    // Back up at 0.4 of the 5.1 m/s it came down at, so about 0.2 m high.
    expect(highestAfterBounce).toBeGreaterThan(1.15);
    expect(highestAfterBounce).toBeLessThan(1.25);
    // A frame's slide at 4 m/s before the bounce, then the rest at 2 m/s.
    expect(particlesOf(particles)[0]?.position[0]).toBeCloseTo(0.04 + 0.29 * 2);
  });

  it("lets a particle with no restitution fall through the height it was born at", () => {
    const particles = createParticles({ capacity: 4, gravity: 10 });
    particles.emit({
      position: [0, 1, 0],
      velocity: [0, -5, 0],
      colour: 0,
      lifetimeSeconds: 5,
      size: 1,
    });
    particles.update(0.1);
    expect(particlesOf(particles)[0]?.position[1]).toBeLessThan(1);
  });

  it("trails a tail behind a particle along its velocity, the stretch's length of travel", () => {
    const particles = createParticles({ capacity: 4, stretchSeconds: 0.03 });
    particles.emit({
      position: [0, 0, 0],
      velocity: [0, 0, 10],
      colour: 0,
      lifetimeSeconds: 1,
      size: 1,
    });
    particles.update(0.1);
    const [streak] = particlesOf(particles);
    expect(streak?.position[2]).toBeCloseTo(1);
    expect(streak?.tail[0]).toBeCloseTo(0);
    expect(streak?.tail[2]).toBeCloseTo(1 - 10 * 0.03);
  });

  it("puts a particle's tail on it when the pool does not stretch", () => {
    const particles = createParticles({ capacity: 4 });
    particles.emit({
      position: [1, 2, 3],
      velocity: [0, 0, 10],
      colour: 0,
      lifetimeSeconds: 1,
      size: 1,
    });
    expect(particlesOf(particles)[0]?.tail).toEqual([1, 2, 3]);
  });

  it("reports whether a particle was born glowing", () => {
    const particles = createParticles({ capacity: 4 });
    for (const glow of [true, false, undefined]) {
      particles.emit({
        position: [0, 0, 0],
        velocity: [0, 0, 0],
        colour: 0,
        lifetimeSeconds: 1,
        size: 1,
        ...(glow === undefined ? {} : { glow }),
      });
    }
    expect(particlesOf(particles).map((particle) => particle.glow)).toEqual([true, false, false]);
  });
});
