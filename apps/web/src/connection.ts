// One connection interface over two transports (ADR 0011): WebTransport
// first, carrying each message as a datagram, and WebSocket when
// WebTransport is not ready within the timeout, carrying the same bytes as
// binary messages. Callers never learn which until they ask.

export type Transport = "webtransport" | "websocket";

export interface Connection {
  readonly transport: Transport;
  /** Send one message's bytes; a datagram may be lost, a WebSocket message never. */
  send(bytes: Uint8Array): void;
  /** Called with each message's bytes as it arrives. */
  onMessage(listener: (bytes: Uint8Array) => void): void;
  /** Resolves once the connection has ended, however it ended. */
  readonly closed: Promise<void>;
  close(): void;
}

/** The slice of the browser's WebTransport the connection uses. */
export interface WebTransportLike {
  readonly ready: Promise<unknown>;
  readonly closed: Promise<unknown>;
  readonly datagrams: {
    readonly readable: ReadableStream<Uint8Array>;
    readonly writable: WritableStream<Uint8Array>;
  };
  close(): void;
}

/** The slice of the browser's WebSocket the connection uses. */
export interface WebSocketLike {
  binaryType: string;
  addEventListener<K extends keyof WebSocketEvents>(
    type: K,
    listener: (event: WebSocketEvents[K]) => void,
  ): void;
  send(bytes: Uint8Array<ArrayBuffer>): void;
  close(): void;
}

export interface WebSocketEvents {
  open: unknown;
  close: unknown;
  error: unknown;
  message: { data: unknown };
}

/** The constructors to connect with; the browser's by default, fakes in tests. */
export interface Transports {
  /** Absent in a browser without WebTransport, which goes straight to WebSocket. */
  readonly WebTransport?: (new (url: string) => WebTransportLike) | undefined;
  readonly WebSocket: new (url: string) => WebSocketLike;
}

export interface ConnectOptions {
  /** Where the server terminates QUIC; absent skips WebTransport. */
  readonly webTransportUrl?: string | undefined;
  readonly webSocketUrl: string;
  /** How long WebTransport may take to become ready before the fallback. */
  readonly webTransportTimeoutMs: number;
}

/**
 * Connect over WebTransport, or over WebSocket when WebTransport is
 * unavailable, fails, or is not ready in time. Rejects only when both fail.
 */
export async function connect(
  options: ConnectOptions,
  transports: Transports = browserTransports(),
): Promise<Connection> {
  const { WebTransport } = transports;
  if (options.webTransportUrl !== undefined && WebTransport !== undefined) {
    const transport = new WebTransport(options.webTransportUrl);
    if (await readyWithin(transport, options.webTransportTimeoutMs)) {
      return overWebTransport(transport);
    }
    transport.close();
  }
  return overWebSocket(new transports.WebSocket(options.webSocketUrl));
}

/**
 * Where the server is reached from a page at `location`: QUIC on the same
 * host and port as the page, the WebSocket behind `/api`. Only an HTTPS page
 * offers WebTransport; a plain HTTP page (the Vite dev server) goes straight
 * to the WebSocket, which the dev server proxies.
 */
export function serverUrls(location: { protocol: string; host: string }): {
  webTransportUrl?: string;
  webSocketUrl: string;
} {
  if (location.protocol === "https:") {
    return {
      webTransportUrl: `https://${location.host}/wt`,
      webSocketUrl: `wss://${location.host}/api/ws`,
    };
  }
  return { webSocketUrl: `ws://${location.host}/api/ws` };
}

function browserTransports(): Transports {
  return {
    WebTransport: typeof WebTransport === "undefined" ? undefined : WebTransport,
    WebSocket,
  };
}

/** True once `transport.ready` resolves; false when it rejects or `timeoutMs` passes first. */
function readyWithin(transport: WebTransportLike, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(false), timeoutMs);
    transport.ready.then(
      () => {
        clearTimeout(timer);
        resolve(true);
      },
      () => {
        clearTimeout(timer);
        resolve(false);
      },
    );
  });
}

function overWebTransport(transport: WebTransportLike): Connection {
  const listeners = new Set<(bytes: Uint8Array) => void>();
  const writer = transport.datagrams.writable.getWriter();
  const reader = transport.datagrams.readable.getReader();
  const reading = (async () => {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) return;
      for (const listener of listeners) listener(value);
    }
  })();
  // The session ending is the normal way reading stops; neither is an error here.
  const closed = Promise.race([transport.closed, reading]).then(
    () => undefined,
    () => undefined,
  );
  return {
    transport: "webtransport",
    send(bytes) {
      // Datagrams are fire-and-forget; a write that fails has closed the transport.
      writer.write(bytes).catch(() => {});
    },
    onMessage(listener) {
      listeners.add(listener);
    },
    closed,
    close() {
      transport.close();
    },
  };
}

function overWebSocket(socket: WebSocketLike): Promise<Connection> {
  socket.binaryType = "arraybuffer";
  const listeners = new Set<(bytes: Uint8Array) => void>();
  const closed = new Promise<void>((resolve) => {
    socket.addEventListener("close", () => resolve());
  });
  socket.addEventListener("message", (event) => {
    if (event.data instanceof ArrayBuffer) {
      const bytes = new Uint8Array(event.data);
      for (const listener of listeners) listener(bytes);
    }
  });
  return new Promise((resolve, reject) => {
    socket.addEventListener("error", () => reject(new Error("WebSocket failed to connect")));
    socket.addEventListener("open", () => {
      resolve({
        transport: "websocket",
        send(bytes) {
          socket.send(overArrayBuffer(bytes));
        },
        onMessage(listener) {
          listeners.add(listener);
        },
        closed,
        close() {
          socket.close();
        },
      });
    });
  });
}

/** The same bytes over a plain ArrayBuffer, which is what a WebSocket sends; wasm memory already is one. */
function overArrayBuffer(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  return bytes.buffer instanceof ArrayBuffer
    ? (bytes as Uint8Array<ArrayBuffer>)
    : new Uint8Array(bytes);
}
