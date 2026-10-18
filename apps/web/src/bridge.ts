// The only module that touches the wasm bindings (ADR 0003). Everything else
// sees plain objects: a KartInput in, KartTransforms out. Biome's
// noRestrictedImports rejects any other import of ./generated/bridge.
import init, { Sim, ticks_per_second } from "./generated/bridge/bridge.js";

/** One kart's controls for one tick: steer -1..1, throttle 0..1, brake 0..1. */
export interface KartInput {
  readonly steer: number;
  readonly throttle: number;
  readonly brake: number;
}

/** Position in metres and unit quaternion [x, y, z, w]; Y up, identity faces -Z. */
export interface KartTransform {
  readonly position: readonly [number, number, number];
  readonly rotation: readonly [number, number, number, number];
}

export interface Race {
  readonly kartCount: number;
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

const FLOATS_PER_INPUT = 3;
const FLOATS_PER_TRANSFORM = 7;

/**
 * Load the wasm module once. Pass `wasm` bytes when there is no document
 * (tests); in the browser the bundler resolves the module URL itself.
 */
export async function loadBridge(wasm?: BufferSource): Promise<Bridge> {
  await init(wasm === undefined ? undefined : { module_or_path: wasm });
  return {
    ticksPerSecond: ticks_per_second(),
    createRace(seed, kartCount) {
      return new WasmRace(new Sim(seed, kartCount), kartCount);
    },
  };
}

class WasmRace implements Race {
  readonly #sim: Sim;
  readonly #inputs: Float32Array;

  constructor(
    sim: Sim,
    readonly kartCount: number,
  ) {
    this.#sim = sim;
    this.#inputs = new Float32Array(kartCount * FLOATS_PER_INPUT);
  }

  step(inputs: readonly KartInput[], dt: number): void {
    if (inputs.length !== this.kartCount) {
      throw new RangeError(`expected ${this.kartCount} inputs, got ${inputs.length}`);
    }
    inputs.forEach((input, kart) => {
      const at = kart * FLOATS_PER_INPUT;
      this.#inputs[at] = input.steer;
      this.#inputs[at + 1] = input.throttle;
      this.#inputs[at + 2] = input.brake;
    });
    this.#sim.step(this.#inputs, dt);
  }

  kartTransforms(): KartTransform[] {
    const buffer = this.#sim.kart_transforms();
    const transforms: KartTransform[] = [];
    for (let at = 0; at + FLOATS_PER_TRANSFORM <= buffer.length; at += FLOATS_PER_TRANSFORM) {
      transforms.push({
        position: [buffer[at] ?? 0, buffer[at + 1] ?? 0, buffer[at + 2] ?? 0],
        rotation: [
          buffer[at + 3] ?? 0,
          buffer[at + 4] ?? 0,
          buffer[at + 5] ?? 0,
          buffer[at + 6] ?? 1,
        ],
      });
    }
    return transforms;
  }
}
