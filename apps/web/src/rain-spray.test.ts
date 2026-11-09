import { describe, expect, it } from "vitest";
import type { KartReadout, KartSize } from "./bridge";
import { kartReadout } from "./kart-readout-fixture";
import type { Particles } from "./particles";
import { createRainSpray, createSprayParticles } from "./rain-spray";

const SIZE: KartSize = { width: 1.2, height: 0.6, length: 1.8 };
const CENTRE_Y = SIZE.height / 2;
/** How far up the sim's box the tops of kart.glb's rear wheels stand as the View fits it. */
const WHEEL_TOP_OF_BOX = 0.86;
const FLAT_OUT = kartReadout({ position: [0, CENTRE_Y, 0], speed: 30, surface: "asphalt" });

/** A second of a kart driving down -Z at its speed, the pool moving with it. */
function drive(particles: Particles, kart: KartReadout, wetness: number, seconds = 1) {
  const spray = createRainSpray(particles, SIZE, () => 0.5);
  const dt = 1 / 60;
  let z = 0;
  for (let frame = 0; frame < seconds * 60; frame++) {
    spray.emit({ ...kart, position: [kart.position[0], kart.position[1], z] }, wetness, dt);
    particles.update(dt);
    z -= kart.speed * dt;
  }
  return z;
}

interface Seen {
  count: number;
  highestTop: number;
  furthestBehind: number;
  lowest: number;
}

function looked(particles: Particles, kartZ: number): Seen {
  const seen: Seen = { count: 0, highestTop: 0, furthestBehind: 0, lowest: Infinity };
  particles.forEach((particle) => {
    seen.count += 1;
    seen.highestTop = Math.max(seen.highestTop, particle.position[1] + particle.size / 2);
    seen.lowest = Math.min(seen.lowest, particle.position[1]);
    seen.furthestBehind = Math.max(seen.furthestBehind, particle.position[2] - kartZ);
  });
  return seen;
}

describe("the spray off a kart's rear wheels", () => {
  it("throws nothing on a dry track", () => {
    const particles = createSprayParticles(8);
    drive(particles, FLAT_OUT, 0);
    expect(looked(particles, 0).count).toBe(0);
  });

  it("keeps under the top of the wheels, the budget that leaves wings and helmets clear", () => {
    const particles = createSprayParticles(8);
    const z = drive(particles, FLAT_OUT, 1);
    const seen = looked(particles, z);
    expect(seen.count).toBeGreaterThan(0);
    expect(seen.highestTop).toBeLessThanOrEqual(SIZE.height * WHEEL_TOP_OF_BOX);
    expect(seen.lowest).toBeGreaterThanOrEqual(0);
  });

  it("trails about two kart lengths flat out, and no further on a boost", () => {
    const particles = createSprayParticles(8);
    const z = drive(particles, FLAT_OUT, 1);
    const behindRearAxle = looked(particles, z).furthestBehind - SIZE.length / 2;
    expect(behindRearAxle).toBeGreaterThan(SIZE.length * 1.5);
    expect(behindRearAxle).toBeLessThanOrEqual(SIZE.length * 2);

    const boosted = createSprayParticles(8);
    const further = drive(boosted, { ...FLAT_OUT, speed: 45 }, 1);
    expect(looked(boosted, further).furthestBehind - SIZE.length / 2).toBeLessThanOrEqual(
      SIZE.length * 2,
    );
  });

  it("thickens with the wetness", () => {
    const light = createSprayParticles(8);
    const heavy = createSprayParticles(8);
    expect(looked(light, drive(light, FLAT_OUT, 0.25)).count).toBeLessThan(
      looked(heavy, drive(heavy, FLAT_OUT, 1)).count,
    );
  });

  it("comes off moving tyres on the road alone", () => {
    for (const kart of [
      { ...FLAT_OUT, speed: 0.5 },
      { ...FLAT_OUT, airborne: true },
      { ...FLAT_OUT, surface: "grass" as const },
      { ...FLAT_OUT, surface: "sand" as const },
    ]) {
      const particles = createSprayParticles(8);
      drive(particles, kart, 1);
      expect(looked(particles, 0).count).toBe(0);
    }
  });

  it("holds a kart flat out within its share of the pool, so no plume is cut short", () => {
    const particles = createSprayParticles(1);
    const z = drive(particles, FLAT_OUT, 1, 2);
    expect(looked(particles, z).count).toBeLessThan(particles.capacity);
  });
});
