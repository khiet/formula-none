// The only module that touches the wasm bindings (ADR 0003). Everything else
// sees plain objects: a KartInput in, KartTransforms out. Biome's
// noRestrictedImports rejects any other import of ./generated/bridge.
import init, { ticks_per_second, Race as WasmRace } from "./generated/bridge/bridge.js";

/** One kart's controls for one tick: steer -1..1, throttle 0..1, brake 0..1. */
export interface KartInput {
  readonly steer: number;
  readonly throttle: number;
  readonly brake: number;
}

/**
 * What the View and the Instrument panel need per kart: position in metres,
 * unit quaternion [x, y, z, w] and speed in metres per second. Y up; identity
 * rotation faces -Z.
 */
export interface KartTransform {
  readonly position: readonly [number, number, number];
  readonly rotation: readonly [number, number, number, number];
  readonly speed: number;
}

export interface Race {
  /** Advance one tick of `dt` seconds; one input per kart in grid order. */
  step(inputs: readonly KartInput[], dt: number): void;
  /** One transform per kart in grid order, as of the last step. */
  kartTransforms(): KartTransform[];
}

export interface Bridge {
  /** The sim's fixed tick rate; `step` is meant to be called this often with dt = 1 / rate. */
  readonly ticksPerSecond: number;
  createRace(seed: bigint, kartCount: number): Race;
}

// Buffer layouts the bridge crate documents on Race::step and Race::kart_transforms.
const FLOATS_PER_INPUT = 3;
const FLOATS_PER_TRANSFORM = 8;

/**
 * Load the wasm module once. Pass `wasm` bytes when there is no document
 * (tests); in the browser the bundler resolves the module URL itself.
 */
export async function loadBridge(wasm?: BufferSource): Promise<Bridge> {
  await init(wasm === undefined ? undefined : { module_or_path: wasm });
  return {
    ticksPerSecond: ticks_per_second(),
    createRace(seed, kartCount) {
      return new BridgedRace(new WasmRace(seed, kartCount), kartCount);
    },
  };
}

class BridgedRace implements Race {
  readonly #race: WasmRace;
  readonly #kartCount: number;
  readonly #inputs: Float32Array;

  constructor(race: WasmRace, kartCount: number) {
    this.#race = race;
    this.#kartCount = kartCount;
    this.#inputs = new Float32Array(kartCount * FLOATS_PER_INPUT);
  }

  step(inputs: readonly KartInput[], dt: number): void {
    if (inputs.length !== this.#kartCount) {
      throw new RangeError(`expected ${this.#kartCount} inputs, got ${inputs.length}`);
    }
    inputs.forEach((input, slot) => {
      const at = slot * FLOATS_PER_INPUT;
      this.#inputs[at] = input.steer;
      this.#inputs[at + 1] = input.throttle;
      this.#inputs[at + 2] = input.brake;
    });
    this.#race.step(this.#inputs, dt);
  }

  kartTransforms(): KartTransform[] {
    const buffer = this.#race.kart_transforms();
    if (buffer.length !== this.#kartCount * FLOATS_PER_TRANSFORM) {
      throw new RangeError(
        `transform buffer has ${buffer.length} floats for ${this.#kartCount} karts`,
      );
    }
    const transforms: KartTransform[] = [];
    for (let slot = 0; slot < this.#kartCount; slot++) {
      const at = slot * FLOATS_PER_TRANSFORM;
      const read = (offset: number): number => buffer[at + offset] as number;
      transforms.push({
        position: [read(0), read(1), read(2)],
        rotation: [read(3), read(4), read(5), read(6)],
        speed: read(7),
      });
    }
    return transforms;
  }
}
