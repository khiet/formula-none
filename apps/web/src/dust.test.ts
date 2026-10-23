import { describe, expect, it } from "vitest";
import type { KartSize, KartTransform, Surface } from "./bridge";
import { createDustEmitter, dustTell } from "./dust";
import { createParticles, type Particle } from "./particles";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };
const FRAME_S = 1 / 60;

function kartOn(surface: Surface, speed = 10): KartTransform {
  return {
    position: [0, 0.3, 0],
    rotation: [0, 0, 0, 1],
    speed,
    surface,
    drift: null,
    heldItem: null,
    spinOutSeconds: null,
    airborne: false,
    boostSeconds: null,
  };
}

function particlesOf(particles: ReturnType<typeof createParticles>): Particle[] {
  const out: Particle[] = [];
  particles.forEach((particle) => {
    out.push({ ...particle, position: [...particle.position] });
  });
  return out;
}

describe("dust tell", () => {
  it("puffs over sand and grass in different colours and not over the track", () => {
    const sand = dustTell(kartOn("sand"));
    const grass = dustTell(kartOn("grass"));
    expect(sand).not.toBeNull();
    expect(grass).not.toBeNull();
    expect(sand?.colour).not.toBe(grass?.colour);
    expect(dustTell(kartOn("asphalt"))).toBeNull();
    expect(dustTell(kartOn("wall"))).toBeNull();
    expect(dustTell(kartOn("out of world"))).toBeNull();
  });

  it("puffs whichever way the kart moves and not at a standstill", () => {
    expect(dustTell(kartOn("sand", -5))).not.toBeNull();
    expect(dustTell(kartOn("sand", 0))).toBeNull();
  });
});

describe("dust emitter", () => {
  it("puffs dust off the back of a kart on the run-off, in the surface's colour", () => {
    const particles = createParticles({ capacity: 64 });
    const emitter = createDustEmitter(particles, SIZE, () => 0.5);
    for (let frame = 0; frame < 6; frame++) {
      emitter.emit(kartOn("sand"), FRAME_S);
    }
    const puffs = particlesOf(particles);
    expect(puffs.length).toBeGreaterThan(0);
    for (const puff of puffs) {
      expect(puff.colour).toBe(dustTell(kartOn("sand"))?.colour);
      // Low off the rear bumper; the kart faces -Z so its rear is +Z.
      expect(puff.position[2]).toBeGreaterThan(SIZE.length / 4);
      expect(puff.position[1]).toBeLessThan(0.3);
      expect(Math.abs(puff.position[0])).toBeLessThan(SIZE.width);
    }
  });

  it("puffs nothing on the asphalt or at a standstill", () => {
    const particles = createParticles({ capacity: 64 });
    const emitter = createDustEmitter(particles, SIZE, () => 0.5);
    for (let frame = 0; frame < 30; frame++) {
      emitter.emit(kartOn("asphalt"), FRAME_S);
      emitter.emit(kartOn("grass", 0), FRAME_S);
    }
    expect(particlesOf(particles)).toEqual([]);
  });
});
