// The only module that touches the wasm bindings (ADR 0003). Everything else
// sees plain objects: a KartInput in, KartTransforms out. Biome's
// noRestrictedImports rejects any other import of ./generated/bridge.
import init, {
  countdown_s,
  decode_client_message,
  decode_server_message,
  encode_client_message,
  encode_server_message,
  kart_size_m,
  karts_per_race,
  laps_per_race,
  mini_turbo_tier,
  nickname_max_chars,
  nickname_min_chars,
  ping_resend_s,
  ping_timeout_s,
  protocol_version,
  replay_hash_interval_ticks,
  replay_hashes,
  room_code_length,
  ticks_per_second,
  Race as WasmRace,
  webtransport_connect_timeout_s,
} from "./generated/bridge/bridge.js";
import type { ClientMessage, ServerMessage } from "./generated/protocol";

/**
 * One kart's controls for one tick, whichever device produced them: steer
 * -1..1, throttle 0..1, brake 0..1 (held on its own past a standstill, it
 * backs the kart up), the hop button held, the item button pressed this
 * tick, and the item button held.
 */
export interface KartInput {
  readonly steer: number;
  readonly throttle: number;
  readonly brake: number;
  readonly hop: boolean;
  readonly useItem: boolean;
  readonly holdItem: boolean;
}

/** The box the sim collides with, in metres; the View draws the same box. */
export interface KartSize {
  readonly width: number;
  readonly height: number;
  readonly length: number;
}

/** What is under a kart; the sim's surface tags. */
export type Surface = "asphalt" | "sand" | "grass" | "wall" | "out of world";

/** Which way a drift turns the kart; fixed by the steer at the hop. */
export type DriftDirection = "left" | "right";

/** The mini-turbo tiers a drift charges through, in order. */
export type MiniTurbo = "none" | "blue" | "orange";

/** A drift under way: its direction, how long it has been held and the tier that has charged. */
export interface Drift {
  readonly direction: DriftDirection;
  readonly chargeSeconds: number;
  readonly miniTurbo: MiniTurbo;
}

/**
 * What the View and the Instrument panel need per kart: position in metres,
 * unit quaternion [x, y, z, w], speed in metres per second along the kart's
 * facing (negative backing up), the surface under the kart and the drift
 * under way, if any. Y up; identity rotation faces -Z.
 */
export interface KartTransform {
  readonly position: readonly [number, number, number];
  readonly rotation: readonly [number, number, number, number];
  readonly speed: number;
  readonly surface: Surface;
  readonly drift: Drift | null;
}

/**
 * Where a kart stands in the race, as the Instrument panel shows it: race
 * position from 1, the lap it is on from 1 (staying on the last once the
 * race's laps are done), the last checkpoint crossed in order (0 is the
 * start line), spline progress in metres along the centreline, seconds into
 * the current lap, the best lap once one is completed, and whether the
 * wrong-way warning is up (spline progress has fallen for the tuned time
 * and has not risen since).
 */
export interface Standing {
  readonly position: number;
  readonly lap: number;
  readonly checkpoint: number;
  readonly splineProgress: number;
  readonly lapTimeSeconds: number;
  readonly bestLapSeconds: number | null;
  readonly wrongWay: boolean;
  /** The race clock as the kart finished its laps; null while it is still racing. */
  readonly finishTimeSeconds: number | null;
}

/**
 * Where the race is, as the sim's `Phase` has it: the countdown holding the
 * karts on the grid with the seconds to Go, the race with its clock and,
 * once the first kart has finished, the seconds of grace left before the
 * race ends, or finished, when the standings are the results.
 */
export type Phase =
  | { readonly kind: "countdown"; readonly remainingSeconds: number }
  | {
      readonly kind: "racing";
      readonly elapsedSeconds: number;
      readonly graceLeftSeconds: number | null;
    }
  | { readonly kind: "finished" };

export interface Race {
  /**
   * Advance one tick of `dt` seconds; one input per human kart in grid order.
   * The bots drive themselves.
   */
  step(inputs: readonly KartInput[], dt: number): void;
  /** One transform per kart in grid order, as of the last step. */
  kartTransforms(): KartTransform[];
  /** One standing per kart in grid order, with positions as of the last step. */
  standings(): Standing[];
  /** Where the race is, as of the last step. */
  phase(): Phase;
  /**
   * The sim's hash of everything that influences future ticks, as of the last
   * step. Bit-identical native and in wasm for the same race and inputs,
   * which is what a replay fixture pins (ADR 0002).
   */
  stateHash(): bigint;
  /** What lies under `position` on this race's track; karts are never an answer. */
  surfaceAt(position: readonly [number, number, number]): Surface;
}

export interface PingTiming {
  readonly timeoutMs: number;
  readonly resendMs: number;
}

/** Bounds on a nickname's length in characters, inclusive. */
export interface NicknameChars {
  readonly min: number;
  readonly max: number;
}

/**
 * The protocol's messages as the bridge carries them across the wasm
 * boundary: a value of a generated type in, the postcard bytes out, and back
 * (ADR 0004). The web app never reads or writes the bytes itself.
 */
export interface MessageCodec {
  /** Throws, with the protocol's reason, when `message` is not a ClientMessage. */
  encodeClientMessage(message: ClientMessage): Uint8Array;
  /** Throws, with the protocol's reason, for bytes that are not a ClientMessage. */
  decodeClientMessage(bytes: Uint8Array): ClientMessage;
  /** Throws, with the protocol's reason, when `message` is not a ServerMessage. */
  encodeServerMessage(message: ServerMessage): Uint8Array;
  /** Throws, with the protocol's reason, for bytes that are not a ServerMessage. */
  decodeServerMessage(bytes: Uint8Array): ServerMessage;
}

export interface Bridge extends MessageCodec {
  /**
   * The protocol version this wasm build speaks; the same constant the
   * generated types export (ADR 0013).
   */
  readonly protocolVersion: number;
  /** The sim's fixed tick rate; `step` is meant to be called this often with dt = 1 / rate. */
  readonly ticksPerSecond: number;
  /** The kart's box, read from the sim's tuning so the View never repeats it (ADR 0014). */
  readonly kartSize: KartSize;
  /** Laps in a race, from the sim's tuning (ADR 0014). */
  readonly lapsPerRace: number;
  /** Seconds the karts are held on the grid before Go (ADR 0014). */
  readonly countdownSeconds: number;
  /** The mini-turbo tier a drift held for `chargeSeconds` has reached: the sim's rule. */
  miniTurboTier(chargeSeconds: number): MiniTurbo;
  /**
   * Karts on the grid, from the sim's tuning; bots fill the slots the humans
   * leave (ADR 0014).
   */
  readonly kartsPerRace: number;
  /**
   * A race on the track described by `sidecar` (the JSON exported next to its
   * glTF) with `humanCount` karts on the front of its spawn grid and
   * `botCount` bot-driven karts on the slots behind them. Throws, with the
   * sim's reason, when the sidecar is unusable or has fewer spawn slots than
   * the karts.
   */
  createRace(seed: bigint, humanCount: number, botCount: number, sidecar: string): Race;
  /** A replay's hashes are kept every this many ticks. */
  readonly replayHashIntervalTicks: number;
  /**
   * Rerun a replay (the JSON the sim's `replay` module documents) in wasm on
   * the track described by `sidecar` and report the state hash at every
   * `replayHashIntervalTicks`th tick. Throws, with the sim's reason, when the
   * replay is unreadable or the sidecar is not the replay's track.
   */
  replayHashes(replay: string, sidecar: string): bigint[];
  /** How long WebTransport may take to connect before the WebSocket fallback (ADR 0011). */
  readonly webTransportTimeoutMs: number;
  /** How long a Ping waits for its Pong, and how often it is sent again meanwhile. */
  readonly pingTiming: PingTiming;
  /** How long a nickname may be, from the sim's tuning (ADR 0014). */
  readonly nicknameChars: NicknameChars;
  /** Letters in a room code (ADR 0012). */
  readonly roomCodeLength: number;
}

// Buffer layouts the bridge crate documents on Race::step, Race::kart_transforms,
// Race::standings and Race::phase.
const FLOATS_PER_INPUT = 6;
const FLOATS_PER_TRANSFORM = 12;
const FLOATS_PER_STANDING = 8;
const FLOATS_PER_PHASE = 3;
/** Phase codes in the phase buffer, in the sim's order. */
const PHASES = ["countdown", "racing", "finished"] as const;
/** Surface codes in the transform buffer, as `sim::Surface::code` numbers them. */
const SURFACES: readonly Surface[] = ["asphalt", "sand", "grass", "wall", "out of world"];
/** Drift directions by the buffer's sign: -1 left, 1 right. */
const DRIFT_DIRECTIONS: Readonly<Record<number, DriftDirection>> = { [-1]: "left", 1: "right" };
/** Mini-turbo tiers in the buffer's order. */
const MINI_TURBOS: readonly MiniTurbo[] = ["none", "blue", "orange"];

/**
 * Load the wasm module once. Pass `wasm` bytes when there is no document
 * (tests); in the browser the bundler resolves the module URL itself.
 */
export async function loadBridge(wasm?: BufferSource): Promise<Bridge> {
  await init(wasm === undefined ? undefined : { module_or_path: wasm });
  return {
    protocolVersion: protocol_version(),
    ticksPerSecond: ticks_per_second(),
    kartSize: kartSize(),
    lapsPerRace: laps_per_race(),
    countdownSeconds: countdown_s(),
    miniTurboTier(chargeSeconds) {
      const tier = MINI_TURBOS[mini_turbo_tier(chargeSeconds)];
      if (tier === undefined) {
        throw new RangeError(`unknown mini-turbo tier for ${chargeSeconds} s`);
      }
      return tier;
    },
    kartsPerRace: karts_per_race(),
    replayHashIntervalTicks: replay_hash_interval_ticks(),
    createRace(seed, humanCount, botCount, sidecar) {
      return new BridgedRace(
        rethrown(() => new WasmRace(seed, humanCount, botCount, sidecar)),
        humanCount,
        humanCount + botCount,
      );
    },
    replayHashes(replay, sidecar) {
      return Array.from(rethrown(() => replay_hashes(replay, sidecar)));
    },
    webTransportTimeoutMs: webtransport_connect_timeout_s() * 1000,
    pingTiming: { timeoutMs: ping_timeout_s() * 1000, resendMs: ping_resend_s() * 1000 },
    nicknameChars: { min: nickname_min_chars(), max: nickname_max_chars() },
    roomCodeLength: room_code_length(),
    encodeClientMessage(message) {
      return rethrown(() => encode_client_message(message));
    },
    decodeClientMessage(bytes) {
      return rethrown(() => decode_client_message(bytes) as ClientMessage);
    },
    encodeServerMessage(message) {
      return rethrown(() => encode_server_message(message));
    },
    decodeServerMessage(bytes) {
      return rethrown(() => decode_server_message(bytes) as ServerMessage);
    },
  };
}

/** The three floats the bridge documents on `kart_size_m`: width, height, length. */
function kartSize(): KartSize {
  const [width, height, length] = kart_size_m();
  if (width === undefined || height === undefined || length === undefined) {
    throw new RangeError("the bridge's kart size is not three floats");
  }
  return { width, height, length };
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
  readonly #humanCount: number;
  readonly #kartCount: number;
  readonly #inputs: Float32Array;

  constructor(race: WasmRace, humanCount: number, kartCount: number) {
    this.#race = race;
    this.#humanCount = humanCount;
    this.#kartCount = kartCount;
    this.#inputs = new Float32Array(humanCount * FLOATS_PER_INPUT);
  }

  step(inputs: readonly KartInput[], dt: number): void {
    if (inputs.length !== this.#humanCount) {
      throw new RangeError(`expected ${this.#humanCount} inputs, got ${inputs.length}`);
    }
    inputs.forEach((input, slot) => {
      const at = slot * FLOATS_PER_INPUT;
      this.#inputs[at] = input.steer;
      this.#inputs[at + 1] = input.throttle;
      this.#inputs[at + 2] = input.brake;
      this.#inputs[at + 3] = Number(input.hop);
      this.#inputs[at + 4] = Number(input.useItem);
      this.#inputs[at + 5] = Number(input.holdItem);
    });
    this.#race.step(this.#inputs, dt);
  }

  stateHash(): bigint {
    return this.#race.state_hash();
  }

  surfaceAt([x, y, z]: readonly [number, number, number]): Surface {
    const code = this.#race.surface_at(x, y, z);
    const surface = SURFACES[code];
    if (surface === undefined) {
      throw new RangeError(`unknown surface code ${code}`);
    }
    return surface;
  }

  standings(): Standing[] {
    const buffer = this.#race.standings();
    if (buffer.length !== this.#kartCount * FLOATS_PER_STANDING) {
      throw new RangeError(
        `standings buffer has ${buffer.length} floats for ${this.#kartCount} karts`,
      );
    }
    const standings: Standing[] = [];
    for (let slot = 0; slot < this.#kartCount; slot++) {
      const at = slot * FLOATS_PER_STANDING;
      const read = (offset: number): number => buffer[at + offset] as number;
      standings.push({
        position: read(0),
        lap: read(1),
        checkpoint: read(2),
        splineProgress: read(3),
        lapTimeSeconds: read(4),
        bestLapSeconds: timeOrNull(read(5)),
        wrongWay: read(6) !== 0,
        finishTimeSeconds: timeOrNull(read(7)),
      });
    }
    return standings;
  }

  phase(): Phase {
    const buffer = this.#race.phase();
    const [code, seconds, grace] = buffer;
    if (
      buffer.length !== FLOATS_PER_PHASE ||
      code === undefined ||
      seconds === undefined ||
      grace === undefined
    ) {
      throw new RangeError(`phase buffer has ${buffer.length} floats`);
    }
    const kind = PHASES[code];
    switch (kind) {
      case "countdown":
        return { kind, remainingSeconds: seconds };
      case "racing":
        return { kind, elapsedSeconds: seconds, graceLeftSeconds: timeOrNull(grace) };
      case "finished":
        return { kind };
      default:
        throw new RangeError(`unknown phase code ${code}`);
    }
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
        drift: drift(read(9), read(10), read(11), slot),
      });
    }
    return transforms;
  }
}

/** The bridge writes a negative time where there is none yet: no best lap, no finish, no grace. */
function timeOrNull(seconds: number): number | null {
  return seconds < 0 ? null : seconds;
}

/** The drift three buffer floats describe, or null when the direction is 0. */
function drift(direction: number, tier: number, chargeSeconds: number, slot: number): Drift | null {
  if (direction === 0) {
    return null;
  }
  const turning = DRIFT_DIRECTIONS[direction];
  const miniTurbo = MINI_TURBOS[tier];
  if (turning === undefined || miniTurbo === undefined) {
    throw new RangeError(`unknown drift ${direction} at tier ${tier} for kart ${slot}`);
  }
  return { direction: turning, chargeSeconds, miniTurbo };
}
