// A room's race kept connected through a lost connection (ADR 0012): once
// the server has been silent for the tuned time, or the connection ends,
// the connection banner goes up and the client rejoins the room over a new
// connection with its session token, trying again at the tuned interval
// until it is back. Meanwhile a bot drives the kart on the server. It gives
// up only on an answer: a refused rejoin, or another connection taking the
// seat. The Room Scene keeps its connection the same way through a server
// restart (ADR 0053), but on a closed connection alone: between races the
// server frees the seat of a connection that drops, so giving up a quiet
// one to rejoin would cost the seat.

import type { Tuning } from "./bridge";
import type { Connection } from "./connection";
import { RefusedError, type RoomClient, type Unsubscribe } from "./room-client";

export interface ReconnectTiming {
  /** Silence from the server mid-race that counts as a lost connection. */
  readonly silenceMs: number;
  /** The wait after a failed reconnect before the next try. */
  readonly retryMs: number;
}

/** When a race's connection counts as lost, and how often it is tried again, from the sim's seconds. */
export function reconnectTiming(
  tuning: Pick<Tuning, "connectionSilenceS" | "reconnectRetryS">,
): ReconnectTiming {
  return { silenceMs: tuning.connectionSilenceS * 1000, retryMs: tuning.reconnectRetryS * 1000 };
}

/** Why the race's connection is gone for good. */
export type Gone =
  /** The room or the seat is no more: the server restarted, or the race ended without us. */
  | "refused"
  /** Another tab or device rejoined with this session's token. */
  | "replaced";

export interface KeepConnectedOptions {
  readonly timing: ReconnectTiming;
  /** A new connection to the server. */
  connect(): Promise<Connection>;
  /** The clock silence is timed on, in milliseconds; `performance.now` by default. */
  readonly now?: () => number;
  /**
   * Whether the server's silence counts as a lost connection, as it does
   * mid-race; false, and only the connection ending does, as in the Room
   * Scene. True by default.
   */
  readonly silence?: boolean;
}

export interface RaceLink {
  /** Called with true when the connection counts as lost, and false once the seat is back. */
  onLost(listener: (lost: boolean) => void): Unsubscribe;
  /** Resolves once the connection is gone for good, with why. */
  readonly gone: Promise<Gone>;
  /** Stop watching, as when the race is over; a reconnect under way is abandoned. */
  dispose(): void;
}

/** How often within the silence the clock is read, so the banner is late by at most this share, or two after a stall. */
const CHECKS_PER_SILENCE = 8;

export function keepConnected(client: RoomClient, options: KeepConnectedOptions): RaceLink {
  const now = options.now ?? (() => performance.now());
  const { silenceMs, retryMs } = options.timing;
  const listeners = new Set<(lost: boolean) => void>();
  let heardAt = now();
  /** The last check saw silence it could not confirm: it ran late, with the client's own stall behind it. */
  let unconfirmed = false;
  let lost = false;
  let over = false;
  let finish: (gone: Gone) => void = () => {};
  const gone = new Promise<Gone>((resolve) => {
    finish = resolve;
  });

  const stopHeard = client.onHeard(() => {
    heardAt = now();
    unconfirmed = false;
  });
  const report = (next: boolean) => {
    if (lost === next) return;
    lost = next;
    for (const listener of listeners) listener(lost);
  };
  const stop = () => {
    over = true;
    clearInterval(checking);
    stopHeard();
  };
  const end = (why: Gone) => {
    stop();
    finish(why);
  };

  /** Reconnect when the connection the client holds now ends, unless something else already moved on. */
  const watch = () => {
    const closed = client.closed;
    closed.then(() => {
      if (over || lost || closed !== client.closed) return;
      if (client.replaced) {
        end("replaced");
      } else {
        reconnect();
      }
    });
  };

  const reconnect = async () => {
    report(true);
    while (!over) {
      try {
        const connection = await options.connect();
        if (over) {
          connection.close();
          return;
        }
        // A server that never answers ends the try as a closed one does.
        const unanswered = setTimeout(() => connection.close(), silenceMs);
        try {
          await client.resume(connection);
        } finally {
          clearTimeout(unanswered);
        }
        if (over) return;
        heardAt = now();
        report(false);
        watch();
        return;
      } catch (error) {
        if (error instanceof RefusedError) {
          end("refused");
          return;
        }
      }
      await new Promise((resolve) => setTimeout(resolve, retryMs));
    }
  };

  // A check a whole check late was held up by the client itself, a long
  // frame or a paused tab, and the messages that piled up behind the stall
  // may still be unread, so the silence it sees is unconfirmed: it leaves
  // the call to the next check, by when they have landed or nothing has
  // come. Only hearing from the server re-arms the deferral, so a client
  // slow at every check still catches a silent server within two checks of
  // the tuned silence, while one kept slow by a live server never reads its
  // own stalls as the server's.
  const interval = silenceMs / CHECKS_PER_SILENCE;
  let checkedAt = now();
  const checking =
    options.silence === false
      ? undefined
      : setInterval(() => {
          const at = now();
          const stalled = at - checkedAt >= 2 * interval;
          checkedAt = at;
          if (lost || over) return;
          if (stalled && !unconfirmed) {
            unconfirmed = true;
            return;
          }
          unconfirmed = false;
          if (at - heardAt >= silenceMs) reconnect();
        }, interval);
  watch();

  return {
    onLost(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    gone,
    dispose() {
      stop();
      listeners.clear();
    },
  };
}
