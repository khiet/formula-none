// The client's side of a room (ADR 0012) and of the race it runs (ADR
// 0009): join over a connection, follow the room through its updates, send
// the player's ready, Start, Rematch and Leave, and once the creator starts
// the race hand on the Countdown, every snapshot in tick order, every
// event once and the results, while sending the player's inputs with a
// sequence the snapshots acknowledge. Messages travel as the bridge
// encodes them; this module never reads the bytes.

import type { KartInput, MessageCodec } from "./bridge";
import type { Connection } from "./connection";
import type { Event, FinalStanding, Refusal, Room, Snapshot } from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";
import { normaliseCode } from "./room-link";

/**
 * The race starting: every client builds the sim from these (ADR 0002).
 * `grid` is the room slot of each human kart in grid order; the karts
 * behind them are bots.
 */
export interface Countdown {
  readonly seed: bigint;
  readonly track: string;
  readonly grid: readonly number[];
}

/** Stops the listener; calling it again does nothing. */
export type Unsubscribe = () => void;

export interface RoomClient {
  /** This player's slot, which names them in every room update. */
  readonly slot: number;
  /** The room as of its last update. */
  readonly room: Room;
  onRoom(listener: (room: Room) => void): Unsubscribe;
  onCountdown(listener: (countdown: Countdown) => void): Unsubscribe;
  /** Each snapshot newer than the last one delivered; a late or repeated one is dropped. */
  onSnapshot(listener: (snapshot: Snapshot) => void): Unsubscribe;
  /** Each event once, whether it came on the reliable lane or in a snapshot's repeat (ADR 0011). */
  onEvent(listener: (event: Event) => void): Unsubscribe;
  onResults(listener: (standings: readonly FinalStanding[]) => void): Unsubscribe;
  /** Resolves once the connection has ended, however it ended. */
  readonly closed: Promise<void>;
  ready(ready: boolean): void;
  start(): void;
  /** Back to the room for another race. */
  rematch(): void;
  /**
   * Send one tick's controls on the datagram lane under the next sequence,
   * which is returned; the sequence restarts at 1 with each Countdown.
   */
  sendInput(input: KartInput): number;
  /** Leave the room and close the connection; a Join is the first message on a connection. */
  leave(): void;
}

export interface JoinRequest {
  /** As typed; sent in the server's form. */
  readonly code: string;
  readonly nickname: string;
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
  return new Promise((resolve, reject) => {
    let room: Room | undefined;
    let slot = 0;
    const roomListeners = new Set<(room: Room) => void>();
    const countdownListeners = new Set<(countdown: Countdown) => void>();
    const snapshotListeners = new Set<(snapshot: Snapshot) => void>();
    const eventListeners = new Set<(event: Event) => void>();
    const resultsListeners = new Set<(standings: readonly FinalStanding[]) => void>();
    const subscribe =
      <L>(listeners: Set<L>) =>
      (listener: L): Unsubscribe => {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      };

    // The race's bookkeeping, reset by each Countdown. Event ids count up
    // for the room's whole stay, across its races, so once a race is over
    // every id up to its highest is spent: a late snapshot repeating one
    // after the next Countdown is dropped without remembering the id.
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

    connection.onMessage((bytes) => {
      let message: ReturnType<MessageCodec["decodeServerMessage"]>;
      try {
        message = codec.decodeServerMessage(bytes);
      } catch {
        // Not a message; the server sends only messages, so this is noise on the wire.
        return;
      }
      if ("joined" in message) {
        if (room !== undefined) return;
        room = message.joined.room;
        slot = message.joined.slot;
        resolve(client());
      } else if ("refused" in message) {
        reject(new RefusedError(message.refused));
      } else if ("room" in message) {
        room = message.room;
        for (const listener of roomListeners) listener(room);
      } else if ("countdown" in message) {
        newestTick = 0;
        sequence = 0;
        spentEventIds = highestEventId;
        seenEvents.clear();
        const countdown: Countdown = {
          seed: BigInt(message.countdown.seed),
          track: message.countdown.track,
          grid: message.countdown.grid,
        };
        for (const listener of countdownListeners) listener(countdown);
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
        for (const listener of resultsListeners) listener(message.results.standings);
      }
    });
    connection.closed.then(() => reject(new Error("connection closed before the room answered")));

    const client = (): RoomClient => ({
      slot,
      get room() {
        return room as Room;
      },
      onRoom: subscribe(roomListeners),
      onCountdown: subscribe(countdownListeners),
      onSnapshot: subscribe(snapshotListeners),
      onEvent: subscribe(eventListeners),
      onResults: subscribe(resultsListeners),
      closed: connection.closed,
      ready(ready) {
        connection.send(codec.encodeClientMessage({ ready: { ready } }));
      },
      start() {
        connection.send(codec.encodeClientMessage("start"));
      },
      rematch() {
        connection.send(codec.encodeClientMessage("rematch"));
      },
      sendInput(input) {
        sequence += 1;
        connection.send(
          codec.encodeClientMessage({ input: quantiseInput(input, sequence) }),
          "datagram",
        );
        return sequence;
      },
      leave() {
        connection.send(codec.encodeClientMessage("leave"));
        connection.close();
      },
    });

    connection.send(
      codec.encodeClientMessage({
        join: {
          version: PROTOCOL_VERSION,
          room: normaliseCode(request.code),
          nickname: request.nickname,
        },
      }),
    );
  });
}
