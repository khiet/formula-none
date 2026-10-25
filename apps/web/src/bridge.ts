// The only module that touches the wasm bindings (ADR 0003). Everything else
// sees plain objects: a KartInput in, KartReadouts out. Biome's
// noRestrictedImports rejects any other import of ./generated/bridge.
import init, {
  decode_client_message,
  decode_server_message,
  encode_client_message,
  encode_server_message,
  protocol_version,
  replay_hashes,
  tuning,
  Prediction as WasmPrediction,
  Race as WasmRace,
} from "./generated/bridge/bridge.js";
import type { ClientMessage, Input, ServerMessage, Snapshot } from "./generated/protocol";
import type { KartReadout as SimKartReadout, Surface, Tuning } from "./generated/sim";

export type { Drift, DriftDirection, MiniTurbo, Surface } from "./generated/sim";

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

/**
 * What the View and the Instrument panel read per kart: the sim's kart
 * readout (GLOSSARY), its transform in metres and as a unit quaternion
 * [x, y, z, w], and the surface under it. Y up; identity rotation faces -Z.
 * The generated type documents the rest; a field the sim adds to the
 * readout reaches here without a change to this module.
 */
export type KartReadout = SimKartReadout & {
  readonly position: readonly [number, number, number];
  readonly rotation: readonly [number, number, number, number];
  readonly surface: Surface;
};

/** Where an item box stands on the track: metres, and a yaw about +Y, zero facing -Z. */
export interface ItemBox {
  readonly position: readonly [number, number, number];
  readonly yaw: number;
}

/**
 * A hazard on the track as the View draws it: its centre on the surface in
 * metres, the kart that fired it, and whether that kart still trails it.
 */
export interface Hazard {
  readonly position: readonly [number, number, number];
  readonly owner: number;
  readonly held: boolean;
}

/**
 * A homing projectile in flight as the View draws it: its centre in metres,
 * the kart that fired it and the kart it chases.
 */
export interface Projectile {
  readonly position: readonly [number, number, number];
  readonly owner: number;
  readonly target: number;
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
  /** One readout per kart in grid order, as of the last step. */
  kartReadouts(): KartReadout[];
  /** One standing per kart in grid order, with positions as of the last step. */
  standings(): Standing[];
  /** Where the race is, as of the last step. */
  phase(): Phase;
  /** The track's item boxes, in the sidecar's order; fixed for the race. */
  itemBoxes(): ItemBox[];
  /** Whether each item box, in the same order, is there to pick up as of the last step. */
  itemBoxesPresent(): boolean[];
  /** The hazards on the track as of the last step, in the order they were fired. */
  hazards(): Hazard[];
  /** The projectiles in flight as of the last step, in the order they were fired. */
  projectiles(): Projectile[];
  /**
   * The sim's hash of everything that influences future ticks, as of the last
   * step. Bit-identical native and in wasm for the same race and inputs,
   * which is what a replay fixture pins (ADR 0002).
   */
  stateHash(): bigint;
}

/**
 * The player's own kart in a room's race, predicted in this browser by the
 * sim the server runs (ADR 0009): stepped with each input the client sends,
 * and reconciled with each snapshot it receives, any correction blended out
 * over the tuned ticks. The other karts are interpolated between the
 * snapshots, the tuned delay behind the newest, and the own kart meets them
 * there as bodies it cannot push.
 */
export interface Prediction {
  /**
   * Run the interpolation a tick on, then predict one tick from the input
   * the client sends the server this tick.
   */
  tick(input: Input): void;
  /**
   * Reconcile with a snapshot and hand it to the interpolation; returns how
   * far the predicted kart moved for it, in metres.
   */
  reconcile(snapshot: Snapshot): number;
  /**
   * The kart as it is drawn, as of the last tick or snapshot. The
   * prediction races it alone, so its race position counts only itself.
   */
  kartReadout(): KartReadout;
  /**
   * Every kart in grid order where the interpolation draws it, the own kart
   * by the snapshots rather than the prediction; empty before the first
   * snapshot.
   */
  interpolatedKartReadouts(): KartReadout[];
  /**
   * Every projectile in flight where the interpolation draws it, between
   * the snapshots as the other karts are; empty before the first snapshot.
   */
  interpolatedProjectiles(): Projectile[];
  /**
   * The race clock at the predicted kart's tick: the countdown the
   * player's inputs meet on the server. Never finished; the server's
   * results end the race.
   */
  phase(): Phase;
  /** Snapshots that moved the predicted kart further than rounding, per second raced. */
  correctionsPerSecond(): number;
}

export interface PingTiming {
  readonly timeoutMs: number;
  readonly resendMs: number;
}

export interface ReconnectTiming {
  /** Silence from the server mid-race that counts as a lost connection. */
  readonly silenceMs: number;
  /** The wait after a failed reconnect before the next try. */
  readonly retryMs: number;
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

/** What the chase camera reads from the sim's tuning (ADR 0020): the speed its pull-back is scaled to, and the hit's shake. */
export interface CameraTuning {
  readonly topSpeedMetresPerSecond: number;
  /** How much further back the camera sits at top speed than at a standstill. */
  readonly pullBackMetres: number;
  /** How long the camera shakes after the own kart is hit. */
  readonly shakeSeconds: number;
  /** How far the shake throws the camera as it starts. */
  readonly shakeAmplitudeMetres: number;
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
  /** Seconds a picked-up item box is gone before it is back (ADR 0014). */
  readonly itemBoxRespawnSeconds: number;
  /** How long a fire predicted from a press shows before the server is taken not to have fired (ADR 0014). */
  readonly predictedFireTimeoutSeconds: number;
  /** Metres behind a kart a trailed hazard rides (ADR 0014). */
  readonly hazardDropDistanceMetres: number;
  /** How long a spin-out lasts (ADR 0014), which the spin animation runs once over. */
  readonly spinOutSeconds: number;
  /** The chase camera's pull-back and shake, from the sim's tuning (ADR 0020). */
  readonly cameraTuning: CameraTuning;
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
  /**
   * The prediction of the kart on grid slot `kart` in the race from `seed`
   * on the track described by `sidecar`, at the top of its countdown.
   * Throws, with the sim's reason, when the sidecar is unusable or its grid
   * has no such slot.
   */
  createPrediction(seed: bigint, kart: number, sidecar: string): Prediction;
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
  /** When a race's connection counts as lost, and how often it is tried again (ADR 0012). */
  readonly reconnectTiming: ReconnectTiming;
  /** How long a nickname may be, from the sim's tuning (ADR 0014). */
  readonly nicknameChars: NicknameChars;
  /** Letters in a room code (ADR 0012). */
  readonly roomCodeLength: number;
}

// Buffer layouts the bridge crate documents on Race::step, Race::kart_transforms,
// Race::standings and Race::phase; a Prediction's transform and phase are laid
// out as a Race's.
const FLOATS_PER_INPUT = 6;
const FLOATS_PER_TRANSFORM = 7;
const FLOATS_PER_ITEM_BOX = 4;
const FLOATS_PER_HAZARD = 5;
const FLOATS_PER_PROJECTILE = 5;
const FLOATS_PER_STANDING = 8;
const FLOATS_PER_PHASE = 3;
/** Phase codes in the phase buffer, in the sim's order. */
const PHASES = ["countdown", "racing", "finished"] as const;

/** One kart's readout less its transform, and the surface under it, as the bridge pairs them. */
type ReadoutPair = [SimKartReadout, Surface];

/**
 * Load the wasm module once. Pass `wasm` bytes when there is no document
 * (tests); in the browser the bundler resolves the module URL itself.
 */
export async function loadBridge(wasm?: BufferSource): Promise<Bridge> {
  await init(wasm === undefined ? undefined : { module_or_path: wasm });
  const tuned = rethrown(() => tuning() as Tuning);
  const [width, height, length] = tuned.kartSizeM;
  return {
    protocolVersion: protocol_version(),
    ticksPerSecond: tuned.ticksPerSecond,
    kartSize: { width, height, length },
    lapsPerRace: tuned.lapsPerRace,
    countdownSeconds: tuned.countdownS,
    itemBoxRespawnSeconds: tuned.itemBoxRespawnS,
    predictedFireTimeoutSeconds: tuned.predictedFireTimeoutS,
    hazardDropDistanceMetres: tuned.hazardDropDistanceM,
    spinOutSeconds: tuned.spinOutS,
    cameraTuning: {
      topSpeedMetresPerSecond: tuned.topSpeedMPerS,
      pullBackMetres: tuned.cameraPullBackM,
      shakeSeconds: tuned.cameraShakeS,
      shakeAmplitudeMetres: tuned.cameraShakeAmplitudeM,
    },
    kartsPerRace: tuned.kartsPerRace,
    replayHashIntervalTicks: tuned.replayHashIntervalTicks,
    createRace(seed, humanCount, botCount, sidecar) {
      return new BridgedRace(
        rethrown(() => new WasmRace(seed, humanCount, botCount, sidecar)),
        humanCount,
        humanCount + botCount,
      );
    },
    createPrediction(seed, kart, sidecar) {
      return new BridgedPrediction(
        rethrown(() => new WasmPrediction(seed, kart, sidecar)),
        kart,
      );
    },
    replayHashes(replay, sidecar) {
      return Array.from(rethrown(() => replay_hashes(replay, sidecar)));
    },
    webTransportTimeoutMs: tuned.webtransportConnectTimeoutS * 1000,
    pingTiming: { timeoutMs: tuned.pingTimeoutS * 1000, resendMs: tuned.pingResendS * 1000 },
    reconnectTiming: {
      silenceMs: tuned.connectionSilenceS * 1000,
      retryMs: tuned.reconnectRetryS * 1000,
    },
    nicknameChars: { min: tuned.nicknameMinChars, max: tuned.nicknameMaxChars },
    roomCodeLength: tuned.roomCodeLength,
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
    return phaseFrom(this.#race.phase());
  }

  itemBoxes(): ItemBox[] {
    const buffer = this.#race.item_boxes();
    if (buffer.length % FLOATS_PER_ITEM_BOX !== 0) {
      throw new RangeError(`item box buffer has ${buffer.length} floats`);
    }
    const boxes: ItemBox[] = [];
    for (let at = 0; at < buffer.length; at += FLOATS_PER_ITEM_BOX) {
      const read = (offset: number): number => buffer[at + offset] as number;
      boxes.push({ position: [read(0), read(1), read(2)], yaw: read(3) });
    }
    return boxes;
  }

  itemBoxesPresent(): boolean[] {
    return Array.from(this.#race.item_boxes_present(), (present) => present !== 0);
  }

  hazards(): Hazard[] {
    return recordsFrom(this.#race.hazards(), FLOATS_PER_HAZARD, "hazard", (read) => ({
      position: [read(0), read(1), read(2)],
      owner: read(3),
      held: read(4) !== 0,
    }));
  }

  projectiles(): Projectile[] {
    return projectilesFrom(this.#race.projectiles());
  }

  kartReadouts(): KartReadout[] {
    const karts = readoutsFrom(
      this.#race.kart_transforms(),
      rethrown(() => this.#race.kart_readouts() as ReadoutPair[]),
    );
    if (karts.length !== this.#kartCount) {
      throw new RangeError(`${karts.length} readouts for ${this.#kartCount} karts`);
    }
    return karts;
  }
}

class BridgedPrediction implements Prediction {
  readonly #prediction: WasmPrediction;
  readonly #kart: number;

  constructor(prediction: WasmPrediction, kart: number) {
    this.#prediction = prediction;
    this.#kart = kart;
  }

  tick(input: Input): void {
    rethrown(() => this.#prediction.tick(input));
  }

  reconcile(snapshot: Snapshot): number {
    return rethrown(() => this.#prediction.reconcile(snapshot, this.#kart));
  }

  kartReadout(): KartReadout {
    const [kart] = readoutsFrom(this.#prediction.kart_transform(), [
      rethrown(() => this.#prediction.kart_readout() as ReadoutPair),
    ]);
    if (kart === undefined) {
      throw new RangeError(`no readout for kart ${this.#kart}`);
    }
    return kart;
  }

  interpolatedKartReadouts(): KartReadout[] {
    return readoutsFrom(
      this.#prediction.interpolated_kart_transforms(),
      rethrown(() => this.#prediction.interpolated_kart_readouts() as ReadoutPair[]),
    );
  }

  interpolatedProjectiles(): Projectile[] {
    return projectilesFrom(this.#prediction.interpolated_projectiles());
  }

  phase(): Phase {
    return phaseFrom(this.#prediction.phase());
  }

  correctionsPerSecond(): number {
    return this.#prediction.corrections_per_second();
  }
}

/** The projectiles a buffer of five floats each describes. */
function projectilesFrom(buffer: Float32Array): Projectile[] {
  return recordsFrom(buffer, FLOATS_PER_PROJECTILE, "projectile", (read) => ({
    position: [read(0), read(1), read(2)],
    owner: read(3),
    target: read(4),
  }));
}

/**
 * The records a buffer of `floatsPer` floats each describes, each built
 * from a reader of its floats by offset; `what` names the record in the
 * error for a buffer that does not divide.
 */
function recordsFrom<T>(
  buffer: Float32Array,
  floatsPer: number,
  what: string,
  build: (read: (offset: number) => number) => T,
): T[] {
  if (buffer.length % floatsPer !== 0) {
    throw new RangeError(`${what} buffer has ${buffer.length} floats`);
  }
  const records: T[] = [];
  for (let at = 0; at < buffer.length; at += floatsPer) {
    records.push(build((offset) => buffer[at + offset] as number));
  }
  return records;
}

/** Each kart's readout from `pairs` with its transform from `transforms`; throws when the two disagree on the kart count. */
function readoutsFrom(transforms: Float32Array, pairs: readonly ReadoutPair[]): KartReadout[] {
  if (transforms.length !== pairs.length * FLOATS_PER_TRANSFORM) {
    throw new RangeError(
      `transform buffer has ${transforms.length} floats for ${pairs.length} karts`,
    );
  }
  return pairs.map(([readout, surface], slot) => {
    const read = (offset: number): number =>
      transforms[slot * FLOATS_PER_TRANSFORM + offset] as number;
    return {
      ...readout,
      position: [read(0), read(1), read(2)],
      rotation: [read(3), read(4), read(5), read(6)],
      surface,
    };
  });
}

/** The phase the three floats of a phase buffer describe. */
function phaseFrom(buffer: Float32Array): Phase {
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

/** The bridge writes a negative time where there is none yet: no best lap, no finish, no grace. */
function timeOrNull(seconds: number): number | null {
  return seconds < 0 ? null : seconds;
}
