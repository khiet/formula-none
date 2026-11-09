import { describe, expect, it } from "vitest";
import type { Particles } from "./particles";
import { createRainStreakParticles, createRainStreaks } from "./rain-streaks";

function rain(streaks: number, reducedMotion: boolean, seconds = 2): Particles {
  const particles = createRainStreakParticles();
  const emitter = createRainStreaks(particles, { reducedMotion }, () => 0.5);
  const dt = 1 / 60;
  for (let frame = 0; frame < seconds * 60; frame++) {
    emitter.emit([10, 5, -20], [0, 0, -1], 0, streaks, dt);
    particles.update(dt);
  }
  return particles;
}

function count(particles: Particles): number {
  let live = 0;
  particles.forEach(() => {
    live += 1;
  });
  return live;
}

describe("the rain's streaks", () => {
  it("fall from above and ahead of the camera", () => {
    let seen = 0;
    rain(1, false).forEach((particle) => {
      seen += 1;
      expect(particle.position[2]).toBeLessThan(-20);
      // Thrown from over the camera's head, each going down at its head.
      expect(particle.tail[1]).toBeGreaterThan(particle.position[1]);
    });
    expect(seen).toBeGreaterThan(0);
  });

  it("stay ahead of a camera at speed, so none falls past the lens", () => {
    const particles = createRainStreakParticles();
    const emitter = createRainStreaks(particles, { reducedMotion: false });
    const speed = 45;
    const dt = 1 / 60;
    let z = 0;
    let frames = 0;
    for (; frames < 120; frames++) {
      emitter.emit([0, 3, z], [0, 0, -1], speed, 1, dt);
      particles.update(dt);
      z -= speed * dt;
      particles.forEach((particle) => {
        // Ahead of the camera along its way, past the kart it follows.
        expect(particle.position[2]).toBeLessThan(z - 5);
      });
    }
  });

  it("thin with the level and stop at none, as under a roof", () => {
    expect(count(rain(0, false))).toBe(0);
    expect(count(rain(0.25, false))).toBeLessThan(count(rain(1, false)));
    expect(count(rain(0.25, false))).toBeGreaterThan(0);
  });

  it("are gone soon after the level falls, so a portal shuts them off", () => {
    const particles = rain(1, false);
    const emitter = createRainStreaks(particles, { reducedMotion: false }, () => 0.5);
    for (let frame = 0; frame < 60; frame++) {
      emitter.emit([10, 5, -20], [0, 0, -1], 0, 0, 1 / 60);
      particles.update(1 / 60);
    }
    expect(count(particles)).toBe(0);
  });

  it("are fewer for a viewer who asks for less motion, and still there", () => {
    const less = count(rain(1, true));
    expect(less).toBeGreaterThan(0);
    expect(less).toBeLessThan(count(rain(1, false)));
  });
});
