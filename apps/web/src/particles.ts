import type { Vec3 } from "./kart-frame";

/** What a particle is born with. */
export interface ParticleBirth {
  readonly position: Readonly<Vec3>;
  /** Metres per second in world axes. */
  readonly velocity: Readonly<Vec3>;
  /** A Three.js colour hex. */
  readonly colour: number;
  readonly lifetimeSeconds: number;
  /** Metres across at birth. */
  readonly size: number;
}

/** A live particle as the View draws it. */
export interface Particle {
  readonly position: Readonly<Vec3>;
  readonly colour: number;
  /** Metres across now. */
  readonly size: number;
  /** How much of its life is left: 1 at birth, 0 at death. */
  readonly life: number;
}

export interface ParticleOptions {
  /** How many particles live at once; past that the oldest goes. */
  readonly capacity: number;
  /** Metres per second squared downward. */
  readonly gravity?: number;
  /** The share of its velocity a particle loses per second. */
  readonly drag?: number;
  /** How much of its birth size a particle gains per second. */
  readonly growth?: number;
}

export interface Particles {
  readonly capacity: number;
  emit(birth: ParticleBirth): void;
  /** Move every live particle `dtSeconds` on and age it. */
  update(dtSeconds: number): void;
  /**
   * Every live particle, oldest first within one lap of the store.
   * `particle` is reused between calls: copy what is kept.
   */
  forEach(visit: (particle: Particle, slot: number) => void): void;
}

/**
 * A fixed pool of particles moved by the one rule the pool was made with:
 * velocity under gravity and drag, size growing. A slot is reused as its
 * particle dies or, when the pool is full, as the oldest is replaced, and
 * nothing is allocated per frame, so the tells of eight karts cost the
 * same every frame. Presentation only.
 */
export function createParticles(options: ParticleOptions): Particles {
  const { capacity } = options;
  const gravity = options.gravity ?? 0;
  const drag = options.drag ?? 0;
  const growth = options.growth ?? 0;
  const positions = new Float32Array(capacity * 3);
  const velocities = new Float32Array(capacity * 3);
  const colours = new Uint32Array(capacity);
  const sizes = new Float32Array(capacity);
  const lifetimes = new Float32Array(capacity);
  /** Seconds lived; a slot whose age has reached its lifetime is free. */
  const ages = new Float32Array(capacity);
  let head = 0;
  const particle = { position: [0, 0, 0] as Vec3, colour: 0, size: 0, life: 0 };

  return {
    capacity,
    emit(birth) {
      positions.set(birth.position, head * 3);
      velocities.set(birth.velocity, head * 3);
      colours[head] = birth.colour;
      sizes[head] = birth.size;
      lifetimes[head] = birth.lifetimeSeconds;
      ages[head] = 0;
      head = (head + 1) % capacity;
    },
    update(dtSeconds) {
      const keep = Math.max(1 - drag * dtSeconds, 0);
      for (let slot = 0; slot < capacity; slot++) {
        if ((ages[slot] ?? 0) >= (lifetimes[slot] ?? 0)) {
          continue;
        }
        ages[slot] = (ages[slot] ?? 0) + dtSeconds;
        const at = slot * 3;
        velocities[at + 1] = (velocities[at + 1] ?? 0) - gravity * dtSeconds;
        for (let axis = 0; axis < 3; axis++) {
          velocities[at + axis] = (velocities[at + axis] ?? 0) * keep;
          positions[at + axis] =
            (positions[at + axis] ?? 0) + (velocities[at + axis] ?? 0) * dtSeconds;
        }
      }
    },
    forEach(visit) {
      for (let step = 0; step < capacity; step++) {
        const slot = (head + step) % capacity;
        const age = ages[slot] ?? 0;
        const lifetime = lifetimes[slot] ?? 0;
        if (age >= lifetime) {
          continue;
        }
        const at = slot * 3;
        particle.position[0] = positions[at] ?? 0;
        particle.position[1] = positions[at + 1] ?? 0;
        particle.position[2] = positions[at + 2] ?? 0;
        particle.colour = colours[slot] ?? 0;
        particle.size = (sizes[slot] ?? 0) * (1 + growth * age);
        particle.life = 1 - age / lifetime;
        visit(particle, slot);
      }
    },
  };
}
