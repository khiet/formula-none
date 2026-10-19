import { afterEach, describe, expect, it, vi } from "vitest";
import {
  type ByteStreams,
  connect,
  frame,
  readFrames,
  serverUrls,
  type WebSocketLike,
  type WebTransportLike,
} from "./connection";

const TIMEOUT_MS = 3_000;
const PING = new Uint8Array([0, 42]);
const PONG = new Uint8Array([0, 42]);
const JOIN = new Uint8Array([1, 2, 4, 66, 88, 81, 70]);
const JOINED = new Uint8Array([1, 1, 9]);

/** Resolves or rejects by hand, so a test decides when WebTransport is ready. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** A pair of streams the test feeds and reads: what went in, a way to deliver, and the end. */
function fakeStreams(
  sent: Uint8Array[],
): ByteStreams & { deliver(bytes: Uint8Array): void; end(): void } {
  let incoming!: ReadableStreamDefaultController<Uint8Array>;
  return {
    readable: new ReadableStream<Uint8Array>({
      start: (controller) => {
        incoming = controller;
      },
    }),
    writable: new WritableStream<Uint8Array>({
      write: (chunk) => {
        sent.push(chunk);
      },
    }),
    deliver: (bytes) => incoming.enqueue(bytes),
    end: () => incoming.close(),
  };
}

/** A WebTransport whose readiness, datagrams and reliable stream the test controls. */
class FakeWebTransport implements WebTransportLike {
  static instances: FakeWebTransport[] = [];
  readonly url: string;
  readonly readiness = deferred<void>();
  readonly closing = deferred<void>();
  readonly ready: Promise<void> = this.readiness.promise;
  readonly closed: Promise<void> = this.closing.promise;
  /** Datagrams the client sent. */
  readonly sent: Uint8Array[] = [];
  /** Chunks the client wrote to the reliable stream. */
  readonly sentOnStream: Uint8Array[] = [];
  closeCalls = 0;
  readonly datagrams: ReturnType<typeof fakeStreams>;
  readonly stream: ReturnType<typeof fakeStreams>;
  streamsOpened = 0;

  constructor(url: string) {
    this.url = url;
    this.datagrams = fakeStreams(this.sent);
    this.stream = fakeStreams(this.sentOnStream);
    FakeWebTransport.instances.push(this);
  }

  /** A datagram from the server. */
  receive(bytes: Uint8Array) {
    this.datagrams.deliver(bytes);
  }

  /** Bytes from the server on the reliable stream, as the browser chunks them. */
  receiveOnStream(chunk: Uint8Array) {
    this.stream.deliver(chunk);
  }

  createBidirectionalStream(): Promise<ByteStreams> {
    this.streamsOpened++;
    return Promise.resolve(this.stream);
  }

  close() {
    this.closeCalls++;
    this.closing.resolve();
  }
}

/** A WebSocket whose events the test fires. */
class FakeWebSocket implements WebSocketLike {
  static instances: FakeWebSocket[] = [];
  readonly url: string;
  binaryType = "blob";
  readonly sent: Uint8Array[] = [];
  closeCalls = 0;
  readonly #listeners = new Map<string, Array<(event: never) => void>>();

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }

  addEventListener(type: string, listener: (event: never) => void) {
    this.#listeners.set(type, [...(this.#listeners.get(type) ?? []), listener]);
  }

  #fire(type: string, event?: unknown) {
    for (const listener of this.#listeners.get(type) ?? []) listener(event as never);
  }

  open() {
    this.#fire("open");
  }
  fail() {
    this.#fire("error");
    this.#fire("close");
  }
  /** A binary message from the server, as the browser hands it over. */
  receive(bytes: Uint8Array) {
    this.#fire("message", { data: bytes.buffer.slice(0) });
  }
  send(bytes: Uint8Array) {
    this.sent.push(bytes);
  }
  close() {
    this.closeCalls++;
    this.#fire("close");
  }
}

const URLS = {
  webTransportUrl: "https://example.test/wt",
  webSocketUrl: "wss://example.test/api/ws",
  webTransportTimeoutMs: TIMEOUT_MS,
};
const fakes = { WebTransport: FakeWebTransport, WebSocket: FakeWebSocket };

function received(connection: { onMessage(listener: (bytes: Uint8Array) => void): void }) {
  const messages: Uint8Array[] = [];
  connection.onMessage((bytes) => messages.push(bytes));
  return messages;
}

/** Lets the fake streams' microtasks run. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

afterEach(() => {
  FakeWebTransport.instances = [];
  FakeWebSocket.instances = [];
  vi.useRealTimers();
});

describe("connect", () => {
  it("carries the datagram lane as WebTransport datagrams when it is ready in time", async () => {
    const connecting = connect(URLS, fakes);
    const [transport] = FakeWebTransport.instances;
    expect(transport?.url).toBe(URLS.webTransportUrl);
    transport?.readiness.resolve();
    const connection = await connecting;
    expect(connection.transport).toBe("webtransport");
    expect(FakeWebSocket.instances).toHaveLength(0);

    const messages = received(connection);
    connection.send(PING, "datagram");
    transport?.receive(PONG);
    await settle();
    expect(transport?.sent).toEqual([PING]);
    expect(transport?.sentOnStream).toEqual([]);
    expect(messages).toEqual([PONG]);
    // The stream ending ends the connection, as the session's close does.
    transport?.stream.end();
    await expect(connection.closed).resolves.toBeUndefined();
  });

  it("carries the reliable lane as length-prefixed frames on one bidirectional stream", async () => {
    const connecting = connect(URLS, fakes);
    const [transport] = FakeWebTransport.instances;
    transport?.readiness.resolve();
    const connection = await connecting;
    expect(transport?.streamsOpened).toBe(1);

    const messages = received(connection);
    connection.send(JOIN);
    connection.send(PING);
    await settle();
    expect(transport?.sentOnStream).toEqual([frame(JOIN), frame(PING)]);
    expect(transport?.sent).toEqual([]);
    expect(Array.from(frame(JOIN))).toEqual([7, 0, 0, 0, ...JOIN]);

    // The server's frames arrive however the stream chunks them: two in one
    // read, or one split across reads.
    const both = new Uint8Array([...frame(JOINED), ...frame(PONG)]);
    transport?.receiveOnStream(both);
    await settle();
    expect(messages).toEqual([JOINED, PONG]);
    const framed = frame(JOINED);
    transport?.receiveOnStream(framed.subarray(0, 2));
    await settle();
    expect(messages).toHaveLength(2);
    transport?.receiveOnStream(framed.subarray(2));
    await settle();
    expect(messages).toEqual([JOINED, PONG, JOINED]);
  });

  it("falls back when the session gives no reliable stream", async () => {
    class StreamlessWebTransport extends FakeWebTransport {
      override createBidirectionalStream(): Promise<ByteStreams> {
        return Promise.reject(new Error("no streams"));
      }
    }
    const connecting = connect(URLS, {
      WebTransport: StreamlessWebTransport,
      WebSocket: FakeWebSocket,
    });
    FakeWebTransport.instances[0]?.readiness.resolve();
    await settle();
    expect(FakeWebTransport.instances[0]?.closeCalls).toBe(1);
    FakeWebSocket.instances[0]?.open();
    expect((await connecting).transport).toBe("websocket");
  });

  it("falls back to WebSocket, carrying the same bytes, when WebTransport is not ready within the timeout", async () => {
    vi.useFakeTimers();
    const connecting = connect(URLS, fakes);
    await vi.advanceTimersByTimeAsync(TIMEOUT_MS - 1);
    expect(FakeWebSocket.instances).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1);
    const [transport] = FakeWebTransport.instances;
    expect(transport?.closeCalls).toBe(1);
    const [socket] = FakeWebSocket.instances;
    expect(socket?.url).toBe(URLS.webSocketUrl);
    expect(socket?.binaryType).toBe("arraybuffer");
    socket?.open();
    const connection = await connecting;
    expect(connection.transport).toBe("websocket");

    // Both lanes ride the one channel.
    const messages = received(connection);
    connection.send(PING, "datagram");
    connection.send(JOIN, "reliable");
    socket?.receive(PONG);
    expect(socket?.sent).toEqual([PING, JOIN]);
    expect(messages).toEqual([PONG]);
  });

  it("falls back at once when WebTransport fails to connect", async () => {
    const connecting = connect(URLS, fakes);
    FakeWebTransport.instances[0]?.readiness.reject(new Error("UDP blocked"));
    await settle();
    const [socket] = FakeWebSocket.instances;
    socket?.open();
    expect((await connecting).transport).toBe("websocket");
  });

  it("falls back when the browser refuses the WebTransport session up front", async () => {
    class RefusingWebTransport extends FakeWebTransport {
      constructor(url: string) {
        super(url);
        throw new TypeError("WebTransport is not allowed here");
      }
    }
    const connecting = connect(URLS, {
      WebTransport: RefusingWebTransport,
      WebSocket: FakeWebSocket,
    });
    FakeWebSocket.instances[0]?.open();
    expect((await connecting).transport).toBe("websocket");
  });

  it("goes straight to WebSocket when the browser has no WebTransport", async () => {
    const connecting = connect(URLS, { WebSocket: FakeWebSocket });
    FakeWebSocket.instances[0]?.open();
    expect((await connecting).transport).toBe("websocket");
    expect(FakeWebTransport.instances).toHaveLength(0);
  });

  it("goes straight to WebSocket when no WebTransport URL is given", async () => {
    const connecting = connect({ ...URLS, webTransportUrl: undefined }, fakes);
    FakeWebSocket.instances[0]?.open();
    expect((await connecting).transport).toBe("websocket");
    expect(FakeWebTransport.instances).toHaveLength(0);
  });

  it("rejects when both transports fail", async () => {
    const connecting = connect(URLS, fakes);
    FakeWebTransport.instances[0]?.readiness.reject(new Error("UDP blocked"));
    await settle();
    FakeWebSocket.instances[0]?.fail();
    await expect(connecting).rejects.toThrow(/WebSocket/);
  });

  it("reports the end of either transport through closed", async () => {
    const overTransport = connect(URLS, fakes);
    FakeWebTransport.instances[0]?.readiness.resolve();
    const transportConnection = await overTransport;
    transportConnection.close();
    await expect(transportConnection.closed).resolves.toBeUndefined();

    const overSocket = connect(URLS, { WebSocket: FakeWebSocket });
    FakeWebSocket.instances[0]?.open();
    const socketConnection = await overSocket;
    socketConnection.close();
    await expect(socketConnection.closed).resolves.toBeUndefined();
    expect(FakeWebSocket.instances[0]?.closeCalls).toBe(1);
  });

  it("drops what is sent over a WebSocket after it closed, as a datagram on a closed session is", async () => {
    // A race reconnecting keeps sending its inputs; the browser logs an
    // error for each one a closed socket is handed.
    const connecting = connect(URLS, { WebSocket: FakeWebSocket });
    const socket = FakeWebSocket.instances[0] as FakeWebSocket;
    socket.open();
    const connection = await connecting;
    connection.send(PING);
    socket.fail();
    connection.send(PING, "datagram");
    connection.send(JOIN);
    expect(socket.sent).toEqual([PING]);
  });

  it("drops what is sent over a WebSocket the client is closing", async () => {
    const connecting = connect(URLS, { WebSocket: FakeWebSocket });
    const socket = FakeWebSocket.instances[0] as FakeWebSocket;
    socket.open();
    const connection = await connecting;
    // The browser's close event comes later; the socket is closing meanwhile.
    socket.close = () => {};
    connection.close();
    connection.send(PING);
    expect(socket.sent).toEqual([]);
  });
});

describe("readFrames", () => {
  it("delivers each frame once the stream has all of it and ends with the stream", async () => {
    const streams = fakeStreams([]);
    const delivered: Uint8Array[] = [];
    const reading = readFrames(streams.readable, (bytes) => delivered.push(bytes));
    const framed = frame(JOIN);
    streams.deliver(framed.subarray(0, 1));
    streams.deliver(framed.subarray(1, 5));
    await settle();
    expect(delivered).toEqual([]);
    streams.deliver(new Uint8Array([...framed.subarray(5), ...frame(PONG), ...frame(PING)]));
    await settle();
    expect(delivered).toEqual([JOIN, PONG, PING]);
    streams.deliver(frame(new Uint8Array(0)));
    await settle();
    expect(delivered).toHaveLength(4);
    expect(delivered[3]).toEqual(new Uint8Array(0));
    streams.end();
    await reading;
  });
});

describe("serverUrls", () => {
  it("offers WebTransport on the page's own host and the WebSocket behind /api over HTTPS", () => {
    expect(serverUrls({ protocol: "https:", host: "formulanone.sushimafia.com" })).toEqual({
      webTransportUrl: "https://formulanone.sushimafia.com/wt",
      webSocketUrl: "wss://formulanone.sushimafia.com/api/ws",
    });
  });

  it("offers only the WebSocket from a plain HTTP page", () => {
    expect(serverUrls({ protocol: "http:", host: "localhost:5173" })).toEqual({
      webSocketUrl: "ws://localhost:5173/api/ws",
    });
  });
});
