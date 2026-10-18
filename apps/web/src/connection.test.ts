import { afterEach, describe, expect, it, vi } from "vitest";
import { connect, serverUrls, type WebSocketLike, type WebTransportLike } from "./connection";

const TIMEOUT_MS = 3_000;
const PING = new Uint8Array([0, 42]);
const PONG = new Uint8Array([0, 42]);

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

/** A WebTransport whose readiness and datagrams the test controls. */
class FakeWebTransport implements WebTransportLike {
  static instances: FakeWebTransport[] = [];
  readonly url: string;
  readonly readiness = deferred<void>();
  readonly closing = deferred<void>();
  readonly ready: Promise<void> = this.readiness.promise;
  readonly closed: Promise<void> = this.closing.promise;
  readonly sent: Uint8Array[] = [];
  closeCalls = 0;
  readonly datagrams: WebTransportLike["datagrams"];
  #incoming!: ReadableStreamDefaultController<Uint8Array>;

  constructor(url: string) {
    this.url = url;
    this.datagrams = {
      readable: new ReadableStream<Uint8Array>({
        start: (controller) => {
          this.#incoming = controller;
        },
      }),
      writable: new WritableStream<Uint8Array>({
        write: (chunk) => {
          this.sent.push(chunk);
        },
      }),
    };
    FakeWebTransport.instances.push(this);
  }

  /** A datagram from the server. */
  receive(bytes: Uint8Array) {
    this.#incoming.enqueue(bytes);
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
  it("carries messages over WebTransport datagrams when it is ready in time", async () => {
    const connecting = connect(URLS, fakes);
    const [transport] = FakeWebTransport.instances;
    expect(transport?.url).toBe(URLS.webTransportUrl);
    transport?.readiness.resolve();
    const connection = await connecting;
    expect(connection.transport).toBe("webtransport");
    expect(FakeWebSocket.instances).toHaveLength(0);

    const messages = received(connection);
    connection.send(PING);
    transport?.receive(PONG);
    await settle();
    expect(transport?.sent).toEqual([PING]);
    expect(messages).toEqual([PONG]);
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

    const messages = received(connection);
    connection.send(PING);
    socket?.receive(PONG);
    expect(socket?.sent).toEqual([PING]);
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
