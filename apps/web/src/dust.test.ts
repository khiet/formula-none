import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize, Surface } from "./bridge";
import { createDustEmitter, dustTell } from "./dust";
import { kartReadout } from "./kart-readout-fixture";
import { createParticles, type Particle } from "./particles";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };
const FRAME_S = 1 / 60;
const WORN_BELOW = 30;

function kartOn(surface: Surface, speed = 10): KartReadout {
  return kartReadout({ position: [0, 0.3, 0], speed, surface });
}

function particlesOf(particles: ReturnType<typeof createParticles>): Particle[] {
  const out: Particle[] = [];
  particles.forEach((particle) => {
    out.push({ ...particle, position: [...particle.position] });
  });
  return out;
}

describe("dust tell", () => {
  it("trails light smoke at the wheels of a kart rolling under the cliff", () => {
    const worn = (tyreLife: number, overrides = {}) =>
      dustTell(kartReadout({ speed: 20, tyreLife, ...overrides }), WORN_BELOW);
    expect(worn(30)).toBeNull();
    const smoke = worn(29.9);
    expect(smoke?.at).toBe("wheels");
    expect(smoke?.puffsPerSecond).toBeLessThan(10);
    expect(worn(10, { airborne: true })).toBeNull();
    expect(worn(10, { speed: 0 })).toBeNull();
    expect(worn(10, { surface: "grass" })?.at).toBe("bumper");
  });

  it("puffs over sand and grass in different colours and not over the track", () => {
    const sand = dustTell(kartOn("sand"), WORN_BELOW);
    const grass = dustTell(kartOn("grass"), WORN_BELOW);
    expect(sand).not.toBeNull();
    expect(grass).not.toBeNull();
    expect(sand?.colour).not.toBe(grass?.colour);
    expect(dustTell(kartOn("asphalt"), WORN_BELOW)).toBeNull();
    expect(dustTell(kartOn("wall"), WORN_BELOW)).toBeNull();
    expect(dustTell(kartOn("out_of_world"), WORN_BELOW)).toBeNull();
  });

  it("puffs whichever way the kart moves and not at a standstill", () => {
    expect(dustTell(kartOn("sand", -5), WORN_BELOW)).not.toBeNull();
    expect(dustTell(kartOn("sand", 0), WORN_BELOW)).toBeNull();
  });

  it("puffs at the rear wheels through a stall, even on the asphalt at a standstill", () => {
    const stall = dustTell(stalled(), WORN_BELOW);
    expect(stall).not.toBeNull();
    expect(stall?.at).toBe("wheels");
    expect(stall?.colour).not.toBe(dustTell(kartOn("sand"), WORN_BELOW)?.colour);
    expect(dustTell(kartOn("sand"), WORN_BELOW)?.at).toBe("bumper");
  });
});

/** A kart stalled at Go, on the grid's asphalt. */
function stalled(): KartReadout {
  return { ...kartOn("asphalt", 0), stallS: 1.5 };
}

describe("dust emitter", () => {
  it("puffs dust off the back of a kart on the run-off, in the surface's colour", () => {
    const particles = createParticles({ capacity: 64 });
    const emitter = createDustEmitter(particles, SIZE, WORN_BELOW, () => 0.5);
    for (let frame = 0; frame < 6; frame++) {
      emitter.emit(kartOn("sand"), FRAME_S);
    }
    const puffs = particlesOf(particles);
    expect(puffs.length).toBeGreaterThan(0);
    for (const puff of puffs) {
      expect(puff.colour).toBe(dustTell(kartOn("sand"), WORN_BELOW)?.colour);
      // Low off the rear bumper; the kart faces -Z so its rear is +Z.
      expect(puff.position[2]).toBeGreaterThan(SIZE.length / 4);
      expect(puff.position[1]).toBeLessThan(0.3);
      expect(Math.abs(puff.position[0])).toBeLessThan(SIZE.width);
    }
  });

  it("puffs dust at the rear wheels of a stalled kart", () => {
    const particles = createParticles({ capacity: 64 });
    const emitter = createDustEmitter(particles, SIZE, WORN_BELOW, () => 0.5);
    for (let frame = 0; frame < 6; frame++) {
      emitter.emit(stalled(), FRAME_S);
    }
    const puffs = particlesOf(particles);
    expect(puffs.length).toBeGreaterThan(0);
    for (const puff of puffs) {
      expect(puff.colour).toBe(dustTell(stalled(), WORN_BELOW)?.colour);
      // At a rear wheel: out at the kart's side, low, behind its middle.
      expect(Math.abs(puff.position[0])).toBeGreaterThan(SIZE.width / 4);
      expect(puff.position[1]).toBeLessThan(0.3);
      expect(puff.position[2]).toBeGreaterThan(SIZE.length / 4);
    }
  });

  it("puffs nothing on the asphalt or at a standstill", () => {
    const particles = createParticles({ capacity: 64 });
    const emitter = createDustEmitter(particles, SIZE, WORN_BELOW, () => 0.5);
    for (let frame = 0; frame < 30; frame++) {
      emitter.emit(kartOn("asphalt"), FRAME_S);
      emitter.emit(kartOn("grass", 0), FRAME_S);
    }
    expect(particlesOf(particles)).toEqual([]);
  });
});
