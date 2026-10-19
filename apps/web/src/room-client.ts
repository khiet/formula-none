// The client's side of a room (ADR 0012): join over a connection, follow
// the room through its updates, send the player's ready, Start and Leave,
// and hand the Countdown on when the creator starts the race. Messages
// travel as the bridge encodes them; this module never reads the bytes.

import type { MessageCodec } from "./bridge";
import type { Connection } from "./connection";
import type { Refusal, Room } from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";
import { normaliseCode } from "./room-link";

/** The race starting: every client builds the sim from these (ADR 0002). */
export interface Countdown {
  readonly seed: bigint;
  readonly track: string;
}

export interface RoomClient {
  /** This player's slot, which names them in every room update. */
  readonly slot: number;
  /** The room as of its last update. */
  readonly room: Room;
  onRoom(listener: (room: Room) => void): void;
  onCountdown(listener: (countdown: Countdown) => void): void;
  /** Resolves once the connection has ended, however it ended. */
  readonly closed: Promise<void>;
  ready(ready: boolean): void;
  start(): void;
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
        const countdown: Countdown = {
          seed: BigInt(message.countdown.seed),
          track: message.countdown.track,
        };
        for (const listener of countdownListeners) listener(countdown);
      }
    });
    connection.closed.then(() => reject(new Error("connection closed before the room answered")));

    const client = (): RoomClient => ({
      slot,
      get room() {
        return room as Room;
      },
      onRoom(listener) {
        roomListeners.add(listener);
      },
      onCountdown(listener) {
        countdownListeners.add(listener);
      },
      closed: connection.closed,
      ready(ready) {
        connection.send(codec.encodeClientMessage({ ready: { ready } }));
      },
      start() {
        connection.send(codec.encodeClientMessage("start"));
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
