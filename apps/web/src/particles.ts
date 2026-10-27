import type { Vec3 } from "./kart-frame";

/** What a particle is born with. */
export interface ParticleBirth {
  readonly position: Readonly<Vec3>;
  /** Metres per second in world axes. */
  readonly velocity: Readonly<Vec3>;
  /** A Three.js colour hex, the particle's colour at birth. */
  readonly colour: number;
  /** The colour it cools to over the pool's cooling share of its life; its birth colour when omitted. */
  readonly cooledColour?: number;
  readonly lifetimeSeconds: number;
  /** Metres across at birth. */
  readonly size: number;
  /**
   * The share of its downward speed a particle keeps, upward, when it falls
   * back to the height it was born at. Omitted or null, it falls through.
   */
  readonly restitution?: number | null;
  /** Whether the View draws a halo round it. */
  readonly glow?: boolean;
}

/** A live particle as the View draws it. */
export interface Particle {
  readonly position: Readonly<Vec3>;
  /**
   * Where the particle was the pool's stretch ago, along its velocity now:
   * the streak's far end. The position itself when the pool does not stretch.
   */
  readonly tail: Readonly<Vec3>;
  /** A Three.js colour hex: the colour now, partway from its birth colour to its cooled one. */
  readonly colour: number;
  /** Metres across now. */
  readonly size: number;
  /** How much of its life is left: 1 at birth, 0 at death. */
  readonly life: number;
  readonly glow: boolean;
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
  /** The share of its life over which a particle cools to its cooled colour; all of it when omitted. */
  readonly coolShare?: number;
  /** Seconds of travel a particle's streak spans; none when omitted. */
  readonly stretchSeconds?: number;
  /** The share of its level velocity a particle keeps when it bounces; all of it when omitted. */
  readonly bounceSlide?: number;
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
  forEach(visit: (particle: Particle) => void): void;
}

/** A restitution that marks a particle as falling through its floor. */
const NO_BOUNCE = -1;

/**
 * A fixed pool of particles moved by the one rule the pool was made with:
 * velocity under gravity and drag, size growing, and a bounce for a
 * particle born with a restitution. A slot is reused as its particle dies
 * or, when the pool is full, as the oldest is replaced, and nothing is
 * allocated per frame, so the tells of eight karts cost the same every
 * frame. Presentation only.
 */
export function createParticles(options: ParticleOptions): Particles {
  const { capacity } = options;
  const gravity = options.gravity ?? 0;
  const drag = options.drag ?? 0;
  const growth = options.growth ?? 0;
  const coolShare = options.coolShare ?? 1;
  const stretchSeconds = options.stretchSeconds ?? 0;
  const bounceSlide = options.bounceSlide ?? 1;
  const positions = new Float32Array(capacity * 3);
  const velocities = new Float32Array(capacity * 3);
  const colours = new Uint32Array(capacity);
  const cooledColours = new Uint32Array(capacity);
  const sizes = new Float32Array(capacity);
  const lifetimes = new Float32Array(capacity);
  /** The height a bouncing particle bounces off: the one it was born at. */
  const floors = new Float32Array(capacity);
  const restitutions = new Float32Array(capacity);
  const glows = new Uint8Array(capacity);
  /** Seconds lived; a slot whose age has reached its lifetime is free. */
  const ages = new Float32Array(capacity);
  let head = 0;
  const particle = {
    position: [0, 0, 0] as Vec3,
    tail: [0, 0, 0] as Vec3,
    colour: 0,
    size: 0,
    life: 0,
    glow: false,
  };

  return {
    capacity,
    emit(birth) {
      positions.set(birth.position, head * 3);
      velocities.set(birth.velocity, head * 3);
      colours[head] = birth.colour;
      cooledColours[head] = birth.cooledColour ?? birth.colour;
      sizes[head] = birth.size;
      lifetimes[head] = birth.lifetimeSeconds;
      floors[head] = birth.position[1];
      restitutions[head] = birth.restitution ?? NO_BOUNCE;
      glows[head] = birth.glow === true ? 1 : 0;
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
        const restitution = restitutions[slot] ?? NO_BOUNCE;
        const floor = floors[slot] ?? 0;
        if (restitution >= 0 && (positions[at + 1] ?? 0) < floor && (velocities[at + 1] ?? 0) < 0) {
          positions[at + 1] = floor;
          velocities[at + 1] = -(velocities[at + 1] ?? 0) * restitution;
          velocities[at] = (velocities[at] ?? 0) * bounceSlide;
          velocities[at + 2] = (velocities[at + 2] ?? 0) * bounceSlide;
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
        for (let axis = 0; axis < 3; axis++) {
          const position = positions[at + axis] ?? 0;
          particle.position[axis] = position;
          particle.tail[axis] = position - (velocities[at + axis] ?? 0) * stretchSeconds;
        }
        particle.colour = mixColour(
          colours[slot] ?? 0,
          cooledColours[slot] ?? 0,
          Math.min(age / (lifetime * coolShare), 1),
        );
        particle.size = (sizes[slot] ?? 0) * (1 + growth * age);
        particle.life = 1 - age / lifetime;
        particle.glow = glows[slot] === 1;
        visit(particle);
      }
    },
  };
}

/** The colour `share` of the way from `from` to `to`, channel by channel, rounded. */
function mixColour(from: number, to: number, share: number): number {
  if (from === to || share <= 0) {
    return from;
  }
  let mixed = 0;
  for (const shift of [16, 8, 0]) {
    const a = (from >> shift) & 0xff;
    const b = (to >> shift) & 0xff;
    mixed |= Math.round(a + (b - a) * share) << shift;
  }
  return mixed;
}
