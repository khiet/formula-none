import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize, MiniTurbo } from "./bridge";
import type { Start } from "./generated/sim";
import { kartReadout } from "./kart-readout-fixture";
import { createParticles, type Particle } from "./particles";
import { createSparkEmitter, createSparkParticles, sparkTell } from "./sparks";

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
    out.push({ ...particle, position: [...particle.position], tail: [...particle.tail] });
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

  it("gives orange quiet sparks, born pale and cooling to orange", () => {
    expect(sparkTell(drifting("orange"))).toEqual({
      tier: "orange",
      colour: 0xfff0b0,
      cooledColour: 0xff7a00,
      perWheelPerSecond: 60,
      speedMetresPerSecond: 6,
      lifetimeSeconds: 0.25,
      glow: false,
      restitution: null,
      burst: 6,
      burstSpreadMetresPerSecond: expect.any(Number),
      burstRiseMaxMetresPerSecond: expect.any(Number),
    });
  });

  it("gives red denser, faster, longer, glowing and bouncing, cooling to red", () => {
    const red = sparkTell(drifting("red"));
    const orange = sparkTell(drifting("orange"));
    expect(red).toEqual({
      tier: "red",
      colour: 0xffd27a,
      cooledColour: 0xe01808,
      perWheelPerSecond: 90,
      speedMetresPerSecond: 7.5,
      lifetimeSeconds: 0.35,
      glow: true,
      restitution: 0.4,
      burst: 20,
      burstSpreadMetresPerSecond: expect.any(Number),
      burstRiseMaxMetresPerSecond: expect.any(Number),
    });
    expect(red?.burstSpreadMetresPerSecond).toBeGreaterThan(
      orange?.burstSpreadMetresPerSecond ?? 0,
    );
  });

  it("shows a rocket start's charge in the tier's colour, with no warning past the full tier", () => {
    expect(sparkTell(STRAIGHT, "clean")).toBeNull();
    expect(sparkTell(STRAIGHT, "weakBoost")).toEqual(sparkTell(drifting("orange")));
    expect(sparkTell(STRAIGHT, "fullBoost")).toEqual(sparkTell(drifting("red")));
    expect(sparkTell(STRAIGHT, "stall")).toEqual(sparkTell(drifting("red")));
  });
});

describe("spark emitter", () => {
  it("throws sparks off the rear wheels, backward and low, while a tier is charged", () => {
    const particles = createParticles({ capacity: 64, gravity: 9.8 });
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    emitter.emit(kart(drifting("orange")), FRAME_S);
    const born = particlesOf(particles);
    expect(born.length).toBeGreaterThan(0);
    for (const spark of born) {
      expect(spark.colour).toBe(0xfff0b0);
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
    expect([...colours]).toEqual([sparkTell(drifting("red"))?.colour]);

    // From Go the verdict stands on the transform, but the charge is over.
    const after = createParticles({ capacity: 64, gravity: 9.8 });
    const later = createSparkEmitter(after, SIZE, fixedRandom());
    for (let frame = 0; frame < 30; frame++) {
      later.emit(kart(STRAIGHT, "fullBoost"), FRAME_S, false);
      later.emit(kart(STRAIGHT, "clean"), FRAME_S, true);
    }
    expect(particlesOf(after)).toEqual([]);
  });

  it("throws red's sparks in red's style: born hot, glowing, and bouncing off the road", () => {
    const particles = createSparkParticles(1);
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    emitter.emit(kart(drifting("red")), FRAME_S);
    const born = particlesOf(particles);
    expect(new Set(born.map((spark) => spark.colour))).toEqual(new Set([0xffd27a]));
    expect(born.every((spark) => spark.glow)).toBe(true);
    for (let frame = 0; frame < 20; frame++) {
      particles.update(FRAME_S);
    }
    for (const spark of particlesOf(particles)) {
      expect(spark.position[1]).toBeGreaterThanOrEqual(0);
      expect(spark.colour).toBe(0xe01808);
    }
  });

  it("throws orange's sparks without a halo", () => {
    const particles = createSparkParticles(1);
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    emitter.emit(kart(drifting("orange")), FRAME_S);
    const born = particlesOf(particles);
    expect(born.length).toBeGreaterThan(0);
    expect(born.some((spark) => spark.glow)).toBe(false);
  });

  it("bursts the tier's sparks on the frame it charges and not on the frames after", () => {
    const particles = createSparkParticles(1);
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    // No time passes, so only a burst throws anything.
    const thrown = (reading: DriftReading, start: Start = "clean", countingDown = false) => {
      const before = particlesOf(particles).length;
      emitter.emit(kart(reading, start), 0, countingDown);
      return particlesOf(particles).length - before;
    };
    expect(thrown(drifting("none"))).toBe(0);
    expect(thrown(drifting("orange"))).toBe(6);
    expect(thrown(drifting("orange"))).toBe(0);
    expect(thrown(drifting("red"))).toBe(20);
    expect(thrown(drifting("red"))).toBe(0);
    // A new drift charges from nothing again.
    expect(thrown(STRAIGHT)).toBe(0);
    expect(thrown(drifting("orange"))).toBe(6);
    expect(thrown(STRAIGHT)).toBe(0);
    // A rocket start's hold charges the same way.
    expect(thrown(STRAIGHT, "weakBoost", true)).toBe(6);
    expect(thrown(STRAIGHT, "fullBoost", true)).toBe(20);
    expect(thrown(STRAIGHT, "stall", true)).toBe(0);
  });

  it("throws a burst faster than the steady sparks", () => {
    const particles = createSparkParticles(1);
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    // The streak spans 0.03 s of flight, so its length gives the speed.
    const speeds = () =>
      particlesOf(particles).map(
        (spark) =>
          Math.hypot(...spark.position.map((at, axis) => at - (spark.tail[axis] ?? 0))) / 0.03,
      );
    emitter.emit(kart(drifting("orange")), 0);
    const burst = speeds();
    emitter.emit(kart(drifting("orange")), FRAME_S);
    const steady = speeds().slice(burst.length);
    expect(steady.length).toBeGreaterThan(0);
    expect(Math.min(...burst)).toBeGreaterThanOrEqual(9);
    expect(Math.max(...steady)).toBeLessThan(9);
  });

  it("draws every spark as a streak trailing back along its flight", () => {
    const particles = createSparkParticles(1);
    createSparkEmitter(particles, SIZE, fixedRandom()).emit(kart(drifting("orange")), FRAME_S);
    for (const spark of particlesOf(particles)) {
      // Thrown backward, +Z, so its tail is ahead of it.
      expect(spark.tail[2]).toBeLessThan(spark.position[2]);
    }
  });

  it("spreads its sparks over the frames at the tier's steady rate", () => {
    const particles = createParticles({ capacity: 512 });
    const emitter = createSparkEmitter(particles, SIZE, fixedRandom());
    // The charge's burst, out of the count.
    emitter.emit(kart(drifting("orange")), 0);
    const burst = particlesOf(particles).length;
    for (let frame = 0; frame < 30; frame++) {
      emitter.emit(kart(drifting("orange")), FRAME_S);
    }
    // Both wheels, 60 a second each, for half a second.
    expect(particlesOf(particles).length - burst).toBeCloseTo(60, -1);
  });
});
