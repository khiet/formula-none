// The client's side of a room (ADR 0012) and of the race it runs (ADR
// 0009): join over a connection, follow the room through its updates, send
// the player's ready, Start, Rematch and Leave, and once the creator starts
// the race hand on the Countdown, every snapshot in tick order, every
// event once and the results, while sending the player's inputs with a
// sequence the snapshots acknowledge. A lost connection mid-race is
// replaced by resuming over a new one with the session token the join
// dealt (ADR 0012): the listeners, the race and its input sequence carry
// on. The server's probe of the connection is answered here, and the race
// report is the one measurement sent back (ADR 0018). Messages travel as
// the bridge encodes them; this module never reads the bytes.

import type { KartInput, MessageCodec } from "./bridge";
import type { Connection } from "./connection";
import type {
  ClientMessage,
  Event,
  FinalStanding,
  Refusal,
  Room,
  Snapshot,
} from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";
import type { Picks } from "./generated/sim";
import { normaliseCode } from "./room-link";

/**
 * The race starting: every client builds the sim from these (ADR 0002).
 * `grid` is the room slot of each human kart in grid order; the karts
 * behind them are bots.
 */
export interface Countdown {
  readonly seed: bigint;
  readonly track: string;
  /** The room's picks, so the prediction, the panel and a replay run the server's race. */
  readonly picks: Picks;
  readonly grid: readonly number[];
}

/** Stops the listener; calling it again does nothing. */
export type Unsubscribe = () => void;

/** What the client measured over a race, for the server's metrics (ADR 0018). */
export interface RaceReport {
  /** Reconciliation corrections of the own kart per second raced. */
  readonly correctionsPerSecond: number;
}

export interface RoomClient {
  /** This player's slot, which names them in every room update. */
  readonly slot: number;
  /** What takes the seat back over a new connection; the player's alone. */
  readonly token: string;
  /** The room as of its last update. */
  readonly room: Room;
  /**
   * The race under way: its Countdown, until its Results. A rejoin
   * mid-race has it before any Scene could listen for the Countdown.
   */
  readonly race: Countdown | undefined;
  /** Another connection took the seat with this session's token; this one is closed. */
  readonly replaced: boolean;
  onRoom(listener: (room: Room) => void): Unsubscribe;
  onCountdown(listener: (countdown: Countdown) => void): Unsubscribe;
  /** Each snapshot newer than the last one delivered; a late or repeated one is dropped. */
  onSnapshot(listener: (snapshot: Snapshot) => void): Unsubscribe;
  /** Each event once, whether it came on the reliable lane or in a snapshot's repeat (ADR 0011). */
  onEvent(listener: (event: Event) => void): Unsubscribe;
  onResults(listener: (standings: readonly FinalStanding[]) => void): Unsubscribe;
  /** Called on every message from the server, so a caller can time the silence between them. */
  onHeard(listener: () => void): Unsubscribe;
  /** Resolves once the current connection has ended, however it ended. */
  readonly closed: Promise<void>;
  /**
   * Take the seat back over `connection` once the current one is lost,
   * which this closes: resolves on the server's Joined, after which every
   * message comes from `connection`, and rejects with a `RefusedError`
   * when refused or when `connection` ends first. The server sends the
   * race under way again, which is not a new race: listeners hear no
   * Countdown and the input sequence runs on.
   */
  resume(connection: Connection): Promise<void>;
  ready(ready: boolean): void;
  start(): void;
  /** Back to the room for another race. */
  rematch(): void;
  /**
   * Send one tick's controls on the datagram lane under the next sequence,
   * which is returned; the sequence restarts at 1 with each Countdown.
   */
  sendInput(input: KartInput): number;
  /** Send the server what this client measured over the race that just ended. */
  report(report: RaceReport): void;
  /** Leave the room and close the connection; a session takes one Join, so another room needs a new connection. */
  leave(): void;
}

export interface JoinRequest {
  /** As typed; sent in the server's form. */
  readonly code: string;
  readonly nickname: string;
}

export interface RejoinRequest {
  /** As typed; sent in the server's form. */
  readonly code: string;
  /** The token a Joined in this room dealt. */
  readonly token: string;
}

/** The server refused the join; `message` is what to show the player. */
export class RefusedError extends Error {
  readonly refusal: Refusal;

  constructor(refusal: Refusal) {
    super(refusalMessage(refusal));
    this.name = "RefusedError";
    this.refusal = refusal;
  }
}

/** What the player reads for each refusal, and what to do about it (ADR 0013). */
export function refusalMessage(refusal: Refusal): string {
  if (typeof refusal === "object") {
    return "Your game is out of date: reload the page";
  }
  switch (refusal) {
    case "unknownRoom":
      return "No room has that code";
    case "roomFull":
      return "That room is full";
    case "roomRacing":
      return "That room is mid-race; join after the race";
    case "unknownSession":
      return "That race is over; join the room again";
    case "sessionInUse":
      return "You are already in that room in another tab";
    default:
      return "The room refused the join";
  }
}

/**
 * The wire form of one tick's controls: each axis to one byte, as the
 * protocol's `Input` carries it. The server undoes this; the sim on both
 * sides sees the same quantised value.
 */
export function quantiseInput(
  input: KartInput,
  sequence: number,
): {
  sequence: number;
  steer: number;
  throttle: number;
  brake: number;
  hop: boolean;
  useItem: boolean;
  holdItem: boolean;
} {
  const unit = (value: number) => Math.round(Math.min(Math.max(value, 0), 1) * 255);
  return {
    sequence,
    steer: Math.round(Math.min(Math.max(input.steer, -1), 1) * 127),
    throttle: unit(input.throttle),
    brake: unit(input.brake),
    hop: input.hop,
    useItem: input.useItem,
    holdItem: input.holdItem,
  };
}

/**
 * Join the room over `connection`: resolves with the room client once the
 * server answers Joined, rejects with a `RefusedError` when it refuses, and
 * rejects when the connection ends first.
 */
export function joinRoom(
  connection: Connection,
  codec: MessageCodec,
  request: JoinRequest,
): Promise<RoomClient> {
  return openRoom(connection, codec, {
    join: {
      version: PROTOCOL_VERSION,
      room: normaliseCode(request.code),
      nickname: request.nickname,
    },
  });
}

/**
 * Take back the seat `request.token` holds, as after the page was closed
 * mid-race: resolves and rejects as `joinRoom` does. A race under way is
 * the client's `race` once the server has sent it.
 */
export function rejoinRoom(
  connection: Connection,
  codec: MessageCodec,
  request: RejoinRequest,
): Promise<RoomClient> {
  return openRoom(connection, codec, rejoinMessage(request.code, request.token));
}

function rejoinMessage(code: string, token: string): ClientMessage {
  return { rejoin: { version: PROTOCOL_VERSION, room: normaliseCode(code), token } };
}

/** A room client whose first message on `connection` is `first`, a Join or a Rejoin. */
function openRoom(
  connection: Connection,
  codec: MessageCodec,
  first: ClientMessage,
): Promise<RoomClient> {
  return new Promise((resolve, reject) => {
    let room: Room | undefined;
    let slot = 0;
    let token = "";
    let race: Countdown | undefined;
    let replaced = false;
    let current = connection;
    /** Settles the Join, Rejoin or resume waiting for the server's answer. */
    let answer:
      | { resolve: () => void; reject: (error: Error) => void; connection: Connection }
      | undefined;
    const roomListeners = new Set<(room: Room) => void>();
    const countdownListeners = new Set<(countdown: Countdown) => void>();
    const snapshotListeners = new Set<(snapshot: Snapshot) => void>();
    const eventListeners = new Set<(event: Event) => void>();
    const resultsListeners = new Set<(standings: readonly FinalStanding[]) => void>();
    const heardListeners = new Set<() => void>();
    const subscribe =
      <L>(listeners: Set<L>) =>
      (listener: L): Unsubscribe => {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      };

    // The race's bookkeeping, reset by each Countdown. Event ids count up
    // for the room's whole stay, so every id up to the last race's highest
    // is spent: a late snapshot repeating one after the next Countdown is
    // dropped without the id being remembered.
    let newestTick = 0;
    let sequence = 0;
    let spentEventIds = 0;
    let highestEventId = 0;
    const seenEvents = new Set<number>();
    const deliverEvent = (event: Event) => {
      if (event.id <= spentEventIds || seenEvents.has(event.id)) return;
      seenEvents.add(event.id);
      highestEventId = Math.max(highestEventId, event.id);
      for (const listener of eventListeners) listener(event);
    };

    const receive = (bytes: Uint8Array) => {
      let message: ReturnType<MessageCodec["decodeServerMessage"]>;
      try {
        message = codec.decodeServerMessage(bytes);
      } catch {
        // Not a message; the server sends only messages, so this is noise on the wire.
        return;
      }
      for (const listener of heardListeners) listener();
      if (typeof message === "object" && "ping" in message) {
        // The server times the round trip, so the answer goes straight
        // back on the lane the probe came on.
        current.send(
          codec.encodeClientMessage({ pong: { nonce: message.ping.nonce } }),
          "datagram",
        );
      } else if (message === "replaced") {
        replaced = true;
        current.close();
      } else if ("joined" in message) {
        room = message.joined.room;
        slot = message.joined.slot;
        token = message.joined.token;
        answer?.resolve();
        answer = undefined;
      } else if ("refused" in message) {
        answer?.reject(new RefusedError(message.refused));
        answer = undefined;
      } else if ("room" in message) {
        room = message.room;
        for (const listener of roomListeners) listener(room);
      } else if ("countdown" in message) {
        const seed = BigInt(message.countdown.seed);
        // A rejoin mid-race is sent the race under way again.
        if (race !== undefined && race.seed === seed) return;
        newestTick = 0;
        sequence = 0;
        spentEventIds = highestEventId;
        seenEvents.clear();
        race = {
          seed,
          track: message.countdown.track,
          picks: message.countdown.picks,
          grid: message.countdown.grid,
        };
        for (const listener of countdownListeners) listener(race);
      } else if ("snapshot" in message) {
        const { snapshot } = message;
        // Events ride the snapshot too, so a lost reliable frame still
        // reaches the listeners; the id says whether they have seen it.
        for (const event of snapshot.events) deliverEvent(event);
        if (snapshot.tick <= newestTick) return;
        newestTick = snapshot.tick;
        for (const listener of snapshotListeners) listener(snapshot);
      } else if ("events" in message) {
        for (const event of message.events) deliverEvent(event);
      } else if ("results" in message) {
        race = undefined;
        for (const listener of resultsListeners) listener(message.results.standings);
      }
    };

    /** Send `message` first on `next` and hear only `next` from now on; settles once the server answers. */
    const attach = (next: Connection, message: ClientMessage): Promise<void> =>
      new Promise((settle, fail) => {
        current = next;
        answer = { resolve: settle, reject: fail, connection: next };
        next.onMessage((bytes) => {
          if (next === current) receive(bytes);
        });
        next.closed.then(() => {
          if (answer?.connection !== next) return;
          answer.reject(new Error("connection closed before the room answered"));
          answer = undefined;
        });
        next.send(codec.encodeClientMessage(message));
      });

    const client: RoomClient = {
      get slot() {
        return slot;
      },
      get token() {
        return token;
      },
      get room() {
        return room as Room;
      },
      get race() {
        return race;
      },
      get replaced() {
        return replaced;
      },
      onRoom: subscribe(roomListeners),
      onCountdown: subscribe(countdownListeners),
      onSnapshot: subscribe(snapshotListeners),
      onEvent: subscribe(eventListeners),
      onResults: subscribe(resultsListeners),
      onHeard: subscribe(heardListeners),
      get closed() {
        return current.closed;
      },
      resume(next) {
        current.close();
        return attach(next, rejoinMessage(client.room.code, token));
      },
      ready(ready) {
        current.send(codec.encodeClientMessage({ ready: { ready } }));
      },
      start() {
        current.send(codec.encodeClientMessage("start"));
      },
      rematch() {
        current.send(codec.encodeClientMessage("rematch"));
      },
      sendInput(input) {
        sequence += 1;
        current.send(
          codec.encodeClientMessage({ input: quantiseInput(input, sequence) }),
          "datagram",
        );
        return sequence;
      },
      report(report) {
        current.send(
          codec.encodeClientMessage({
            raceReport: { correctionsPerSecond: report.correctionsPerSecond },
          }),
        );
      },
      leave() {
        current.send(codec.encodeClientMessage("leave"));
        current.close();
      },
    };

    attach(connection, first).then(() => resolve(client), reject);
  });
}
