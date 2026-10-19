// One connection interface over two transports (ADR 0011): WebTransport
// first, and WebSocket when WebTransport is unavailable or not ready in
// time. A message is sent on a lane: the datagram lane for what the next
// message supersedes (inputs, snapshots), the reliable lane for everything
// else. Over WebTransport the lanes are datagrams and one bidirectional
// stream of length-prefixed frames; over WebSocket both ride the socket.
// Callers see one `Connection`; `transport` says which.

export type Transport = "webtransport" | "websocket";

/** Which delivery a message gets; the WebSocket fallback gives both the same. */
export type Lane = "reliable" | "datagram";

export interface Connection {
  readonly transport: Transport;
  /** Send one message's bytes; on the datagram lane over WebTransport it may be lost. */
  send(bytes: Uint8Array, lane?: Lane): void;
  /** Called with each message's bytes as it arrives, whichever lane it came on. */
  onMessage(listener: (bytes: Uint8Array) => void): void;
  /** Resolves once the connection has ended, however it ended. */
  readonly closed: Promise<void>;
  close(): void;
}

/** A pair of streams, as the browser's WebTransport hands out. */
export interface ByteStreams {
  readonly readable: ReadableStream<Uint8Array>;
  readonly writable: WritableStream<Uint8Array>;
}

/** The slice of the browser's WebTransport the connection uses. */
export interface WebTransportLike {
  readonly ready: Promise<unknown>;
  readonly closed: Promise<unknown>;
  readonly datagrams: ByteStreams;
  createBidirectionalStream(): Promise<ByteStreams>;
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

/** Bytes of the length prefix on each frame of the reliable stream. */
const FRAME_HEADER_BYTES = 4;

/**
 * Connect over WebTransport, or over WebSocket when WebTransport is
 * unavailable, fails, or is not ready in time. Rejects only when the
 * WebSocket fails too.
 */
export async function connect(
  options: ConnectOptions,
  transports: Transports = browserTransports(),
): Promise<Connection> {
  const { WebTransport } = transports;
  if (options.webTransportUrl !== undefined && WebTransport !== undefined) {
    // The constructor itself throws for a URL or policy the browser rejects;
    // that is a fallback case too.
    let transport: WebTransportLike | undefined;
    try {
      transport = new WebTransport(options.webTransportUrl);
    } catch {
      transport = undefined;
    }
    if (transport !== undefined) {
      if (await readyWithin(transport, options.webTransportTimeoutMs)) {
        try {
          return await overWebTransport(transport);
        } catch {
          // No reliable stream: the session is no use, so fall back.
        }
      }
      transport.close();
    }
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

/** `bytes` as one frame of the reliable stream: its length, little-endian, then itself. */
export function frame(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  const framed = new Uint8Array(FRAME_HEADER_BYTES + bytes.length);
  new DataView(framed.buffer).setUint32(0, bytes.length, true);
  framed.set(bytes, FRAME_HEADER_BYTES);
  return framed;
}

/**
 * Hand each complete frame on `readable` to `deliver`, however the stream
 * chunks them: a frame may arrive split across reads or packed with others.
 * Resolves when the stream ends.
 */
export async function readFrames(
  readable: ReadableStream<Uint8Array>,
  deliver: (bytes: Uint8Array) => void,
): Promise<void> {
  const reader = readable.getReader();
  let buffered = new Uint8Array(0);
  for (;;) {
    const { value, done } = await reader.read();
    if (done) return;
    const joined = new Uint8Array(buffered.length + value.length);
    joined.set(buffered);
    joined.set(value, buffered.length);
    buffered = joined;
    for (;;) {
      if (buffered.length < FRAME_HEADER_BYTES) break;
      const length = new DataView(buffered.buffer, buffered.byteOffset).getUint32(0, true);
      if (buffered.length < FRAME_HEADER_BYTES + length) break;
      deliver(buffered.slice(FRAME_HEADER_BYTES, FRAME_HEADER_BYTES + length));
      buffered = buffered.subarray(FRAME_HEADER_BYTES + length);
    }
  }
}

/** Opens the reliable stream; rejects when the session cannot give one. */
async function overWebTransport(transport: WebTransportLike): Promise<Connection> {
  const listeners = new Set<(bytes: Uint8Array) => void>();
  const deliver = (bytes: Uint8Array) => {
    for (const listener of listeners) listener(bytes);
  };
  const stream = await transport.createBidirectionalStream();
  const datagramWriter = transport.datagrams.writable.getWriter();
  const streamWriter = stream.writable.getWriter();
  const datagramReader = transport.datagrams.readable.getReader();
  const readingDatagrams = (async () => {
    for (;;) {
      const { value, done } = await datagramReader.read();
      if (done) return;
      deliver(value);
    }
  })();
  const readingStream = readFrames(stream.readable, deliver);
  // Any of these settling means the session ended, which is not an error here.
  const closed = Promise.race([transport.closed, readingDatagrams, readingStream]).then(
    () => undefined,
    () => undefined,
  );
  return {
    transport: "webtransport",
    send(bytes, lane = "reliable") {
      // A write rejects only once the session has ended, which `closed` already reports.
      if (lane === "datagram") {
        datagramWriter.write(bytes).catch(() => {});
      } else {
        streamWriter.write(frame(bytes)).catch(() => {});
      }
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
  let open = true;
  const closed = new Promise<void>((resolve) => {
    socket.addEventListener("close", () => {
      open = false;
      resolve();
    });
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
        // One channel carries both lanes. Once it has closed a send is
        // dropped, as WebTransport's are, rather than logged by the browser.
        send(bytes) {
          if (open) socket.send(overArrayBuffer(bytes));
        },
        onMessage(listener) {
          listeners.add(listener);
        },
        closed,
        close() {
          open = false;
          socket.close();
        },
      });
    });
  });
}

/** The bytes backed by a plain ArrayBuffer, which `WebSocket.send` takes; copied only when they are not already. */
function overArrayBuffer(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  return bytes.buffer instanceof ArrayBuffer
    ? (bytes as Uint8Array<ArrayBuffer>)
    : new Uint8Array(bytes);
}
