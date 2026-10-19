// A room's race kept connected through a lost connection (ADR 0012): once
// the server has been silent for the tuned time, or the connection ends,
// the connection banner goes up and the client rejoins the room over a new
// connection with its session token, trying again at the tuned interval
// until it is back. Meanwhile a bot drives the kart on the server. It gives
// up only on an answer: a refused rejoin, or another connection taking the
// seat.

import type { ReconnectTiming } from "./bridge";
import type { Connection } from "./connection";
import { RefusedError, type RoomClient, type Unsubscribe } from "./room-client";

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
}

export interface RaceLink {
  /** Called with true when the connection counts as lost, and false once the seat is back. */
  onLost(listener: (lost: boolean) => void): Unsubscribe;
  /** Resolves once the connection is gone for good, with why. */
  readonly gone: Promise<Gone>;
  /** Stop watching, as when the race is over; a reconnect under way is abandoned. */
  dispose(): void;
}

/** How often within the silence the clock is read, so the banner is late by at most this share. */
const CHECKS_PER_SILENCE = 8;

export function keepConnected(client: RoomClient, options: KeepConnectedOptions): RaceLink {
  const now = options.now ?? (() => performance.now());
  const { silenceMs, retryMs } = options.timing;
  const listeners = new Set<(lost: boolean) => void>();
  let heardAt = now();
  let lost = false;
  let over = false;
  let finish: (gone: Gone) => void = () => {};
  const gone = new Promise<Gone>((resolve) => {
    finish = resolve;
  });

  const stopHeard = client.onHeard(() => {
    heardAt = now();
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

  const checking = setInterval(() => {
    if (!lost && !over && now() - heardAt >= silenceMs) reconnect();
  }, silenceMs / CHECKS_PER_SILENCE);
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
