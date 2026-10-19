// A stand-in for a connection to the server, for tests: JSON stands in for
// the bridge's wire format, so a test reads the messages the client sends
// and feeds it the server's replies.

import type { MessageCodec } from "./bridge";
import type { Connection, Lane } from "./connection";
import type { ClientMessage, ServerMessage } from "./generated/protocol";

/** A session token as the server deals them. */
export const TOKEN = "0123456789abcdef0123456789abcdef";

export const jsonCodec: MessageCodec = {
  encodeClientMessage: (message) => new TextEncoder().encode(JSON.stringify(message)),
  decodeClientMessage: (bytes) => JSON.parse(new TextDecoder().decode(bytes)) as ClientMessage,
  encodeServerMessage: (message) => new TextEncoder().encode(JSON.stringify(message)),
  decodeServerMessage: (bytes) => JSON.parse(new TextDecoder().decode(bytes)) as ServerMessage,
};

export interface FakeConnection extends Connection {
  readonly sent: ClientMessage[];
  /** The lane of each message in `sent`. */
  readonly lanes: Lane[];
  /** Deliver a message as if from the server. */
  receive(message: ServerMessage): void;
  /** End the connection from the server's side. */
  end(): void;
  readonly closedByClient: boolean;
}

export function fakeConnection(): FakeConnection {
  const sent: ClientMessage[] = [];
  const lanes: Lane[] = [];
  const listeners = new Set<(bytes: Uint8Array) => void>();
  let end: () => void = () => {};
  const closed = new Promise<void>((resolve) => {
    end = resolve;
  });
  let closedByClient = false;
  return {
    transport: "websocket",
    sent,
    lanes,
    send(bytes, lane = "reliable") {
      sent.push(jsonCodec.decodeClientMessage(bytes));
      lanes.push(lane);
    },
    onMessage(listener) {
      listeners.add(listener);
    },
    closed,
    close() {
      closedByClient = true;
      end();
    },
    receive(message) {
      const bytes = jsonCodec.encodeServerMessage(message);
      for (const listener of listeners) listener(bytes);
    },
    end() {
      end();
    },
    get closedByClient() {
      return closedByClient;
    },
  };
}
