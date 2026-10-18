// The only module that touches the wasm bindings (ADR 0003). Everything else
// sees plain objects: a KartInput in, KartTransforms out. Biome's
// noRestrictedImports rejects any other import of ./generated/bridge.
import init, {
  replay_hash_interval_ticks,
  replay_hashes,
  ticks_per_second,
  Race as WasmRace,
} from "./generated/bridge/bridge.js";

/** One kart's controls for one tick: steer -1..1, throttle 0..1, brake 0..1. */
export interface KartInput {
  readonly steer: number;
  readonly throttle: number;
  readonly brake: number;
}

/** What is under a kart; the sim's surface tags. */
export type Surface = "asphalt" | "sand" | "grass" | "wall" | "out of world";

/**
 * What the View and the Instrument panel need per kart: position in metres,
 * unit quaternion [x, y, z, w], speed in metres per second and the surface
 * under the kart. Y up; identity rotation faces -Z.
 */
export interface KartTransform {
  readonly position: readonly [number, number, number];
  readonly rotation: readonly [number, number, number, number];
  readonly speed: number;
  readonly surface: Surface;
}

export interface Race {
  /** Advance one tick of `dt` seconds; one input per kart in grid order. */
  step(inputs: readonly KartInput[], dt: number): void;
  /** One transform per kart in grid order, as of the last step. */
  kartTransforms(): KartTransform[];
  /**
   * The sim's hash of everything that influences future ticks, as of the last
   * step. Bit-identical native and in wasm for the same race and inputs,
   * which is what a replay fixture pins (ADR 0002).
   */
  stateHash(): bigint;
}

export interface Bridge {
  /** The sim's fixed tick rate; `step` is meant to be called this often with dt = 1 / rate. */
  readonly ticksPerSecond: number;
  /**
   * A race on the track described by `sidecar` (the JSON exported next to its
   * glTF) with `kartCount` karts on its spawn grid. Throws, with the sim's
   * reason, when the sidecar is unusable or has fewer spawn slots than
   * `kartCount`.
   */
  createRace(seed: bigint, kartCount: number, sidecar: string): Race;
  /** A replay's hashes are kept every this many ticks. */
  readonly replayHashIntervalTicks: number;
  /**
   * Rerun a replay (the JSON the sim's `replay` module documents) in wasm on
   * the track described by `sidecar` and report the state hash at every
   * `replayHashIntervalTicks`th tick. Throws, with the sim's reason, when the
   * replay is unreadable or the sidecar is not the replay's track.
   */
  replayHashes(replay: string, sidecar: string): bigint[];
}

// Buffer layouts the bridge crate documents on Race::step and Race::kart_transforms.
const FLOATS_PER_INPUT = 3;
const FLOATS_PER_TRANSFORM = 9;
/** Surface codes in the transform buffer, as `sim::Surface::code` numbers them. */
const SURFACES: readonly Surface[] = ["asphalt", "sand", "grass", "wall", "out of world"];

/**
 * Load the wasm module once. Pass `wasm` bytes when there is no document
 * (tests); in the browser the bundler resolves the module URL itself.
 */
export async function loadBridge(wasm?: BufferSource): Promise<Bridge> {
  await init(wasm === undefined ? undefined : { module_or_path: wasm });
  return {
    ticksPerSecond: ticks_per_second(),
    replayHashIntervalTicks: replay_hash_interval_ticks(),
    createRace(seed, kartCount, sidecar) {
      return new BridgedRace(
        rethrown(() => new WasmRace(seed, kartCount, sidecar)),
        kartCount,
      );
    },
    replayHashes(replay, sidecar) {
      return Array.from(rethrown(() => replay_hashes(replay, sidecar)));
    },
  };
}

/** The bridge rejects with a plain string so its native tests can run; make it an Error. */
function rethrown<T>(call: () => T): T {
  try {
    return call();
  } catch (reason) {
    throw new Error(typeof reason === "string" ? reason : String(reason));
  }
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

  stateHash(): bigint {
    return this.#race.state_hash();
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
      const surface = SURFACES[read(8)];
      if (surface === undefined) {
        throw new RangeError(`unknown surface code ${read(8)} for kart ${slot}`);
      }
      transforms.push({
        position: [read(0), read(1), read(2)],
        rotation: [read(3), read(4), read(5), read(6)],
        speed: read(7),
        surface,
      });
    }
    return transforms;
  }
}
