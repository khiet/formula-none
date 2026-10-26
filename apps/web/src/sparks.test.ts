import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize, MiniTurbo } from "./bridge";
import type { Start } from "./generated/protocol";
import { kartReadout } from "./kart-readout-fixture";
import { createParticles, type Particle } from "./particles";
import { createSparkEmitter, sparkTell } from "./sparks";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };
const FRAME_S = 1 / 60;

type DriftReading = Pick<KartReadout, "drift" | "miniTurbo">;

/** Out of a drift. */
const STRAIGHT: DriftReading = { drift: null, miniTurbo: "none" };

function drifting(miniTurbo: MiniTurbo): DriftReading {
  return { drift: { direction: "right", chargeS: 0 }, miniTurbo };
}

function kart(drift: DriftReading, start: Start = "clean"): KartReadout {
  return kartReadout({ position: [0, 0.3, 0], speed: 25, ...drift, start });
}

function particlesOf(particles: ReturnType<typeof createParticles>): Particle[] {
  const out: Particle[] = [];
  particles.forEach((particle) => {
    out.push({ ...particle, position: [...particle.position] });
  });
  return out;
}

/** A fixed sequence standing in for Math.random, so a test's sparks fly the same way every run. */
function fixedRandom(): () => number {
  let seed = 1;
  return () => {
    seed = (seed * 48271) % 2147483647;
    return seed / 2147483647;
  };
}

describe("spark tell", () => {
  it("shows no sparks off a drift or while the drift has charged nothing yet", () => {
    expect(sparkTell(STRAIGHT)).toBeNull();
    expect(sparkTell(drifting("none"))).toBeNull();
  });

  it("shows sparks once blue has charged and changes colour at orange", () => {
    const blue = sparkTell(drifting("blue"));
    const orange = sparkTell(drifting("orange"));
    expect(blue).not.toBeNull();
    expect(orange).not.toBeNull();
    expect(orange?.colour).not.toBe(blue?.colour);
  });

  it("sparks blue in the shift lights' blue, so blue means one blue", () => {
    expect(sparkTell(drifting("blue"))?.colour).toBe(0x4f7dff);
  });

  it("shows a rocket start's charge in the tier's colour, with no warning past the full tier", () => {
    expect(sparkTell(STRAIGHT, "clean")).toBeNull();
    expect(sparkTell(STRAIGHT, "weakBoost")).toEqual(sparkTell(drifting("blue")));
    expect(sparkTell(STRAIGHT, "fullBoost")).toEqual(sparkTell(drifting("orange")));
    expect(sparkTell(STRAIGHT, "stall")).toEqual(sparkTell(drifting("orange")));
  });
});

describe("spark emitter", () => {
  it("throws sparks off the rear wheels, backward and low, while a tier is charged", () => {
    const particles = createParticles({ capacity: 64, gravity: 9.8 });
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    emitter.emit(kart(drifting("blue")), FRAME_S);
    const born = particlesOf(particles);
    expect(born.length).toBeGreaterThan(0);
    for (const spark of born) {
      expect(spark.colour).toBe(sparkTell(drifting("blue"))?.colour);
      expect(Math.abs(spark.position[0])).toBeLessThan(SIZE.width / 2 + 0.1);
      expect(Math.abs(spark.position[0])).toBeGreaterThan(SIZE.width / 4);
      expect(spark.position[1]).toBeLessThan(0.2);
      expect(spark.position[2]).toBeGreaterThan(0);
    }
    for (let frame = 0; frame < 6; frame++) {
      particles.update(FRAME_S);
    }
    const flown = particlesOf(particles);
    expect(flown).toHaveLength(born.length);
    flown.forEach((spark, index) => {
      // Behind the kart, which faces -Z, is +Z.
      expect(spark.position[2]).toBeGreaterThan((born[index]?.position[2] ?? 0) + 0.1);
    });
  });

  it("throws nothing while the drift is uncharged or over", () => {
    const particles = createParticles({ capacity: 64 });
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    for (let frame = 0; frame < 30; frame++) {
      emitter.emit(kart(drifting("none")), FRAME_S);
      emitter.emit(kart(STRAIGHT), FRAME_S);
    }
    expect(particlesOf(particles)).toEqual([]);
  });

  it("throws sparks for a kart charging its rocket start while the countdown runs, and none after", () => {
    const particles = createParticles({ capacity: 64, gravity: 9.8 });
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    emitter.emit(kart(STRAIGHT, "fullBoost"), FRAME_S, true);
    const colours = new Set(particlesOf(particles).map((spark) => spark.colour));
    expect([...colours]).toEqual([sparkTell(drifting("orange"))?.colour]);

    // From Go the verdict stands on the transform, but the charge is over.
    const after = createParticles({ capacity: 64, gravity: 9.8 });
    const later = createSparkEmitter(after, SIZE, fixedRandom());
    for (let frame = 0; frame < 30; frame++) {
      later.emit(kart(STRAIGHT, "fullBoost"), FRAME_S, false);
      later.emit(kart(STRAIGHT, "clean"), FRAME_S, true);
    }
    expect(particlesOf(after)).toEqual([]);
  });

  it("colours orange's sparks orange", () => {
    const particles = createParticles({ capacity: 64 });
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    emitter.emit(kart(drifting("orange")), FRAME_S);
    const colours = new Set(particlesOf(particles).map((spark) => spark.colour));
    expect([...colours]).toEqual([sparkTell(drifting("orange"))?.colour]);
  });

  it("spreads its sparks over the frames at a steady rate", () => {
    const particles = createParticles({ capacity: 512 });
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    for (let frame = 0; frame < 30; frame++) {
      emitter.emit(kart(drifting("blue")), FRAME_S);
    }
    const halfSecond = particlesOf(particles).length;
    for (let frame = 0; frame < 30; frame++) {
      emitter.emit(kart(drifting("blue")), FRAME_S);
    }
    expect(particlesOf(particles).length).toBeCloseTo(2 * halfSecond, -1);
    expect(halfSecond).toBeGreaterThan(10);
  });
});
