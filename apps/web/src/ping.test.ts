import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { loadBridge } from "./bridge";
import type { Connection } from "./connection";
import { ping } from "./ping";

// The real wasm build encodes the Ping and decodes the Pong, so the bytes
// the browser sends are the ones the server's tests answer.
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));

/** A connection whose far end the test plays. */
function fakeConnection() {
  const listeners: Array<(bytes: Uint8Array) => void> = [];
  const sent: Uint8Array[] = [];
  const connection: Connection = {
    transport: "websocket",
    send(bytes) {
      sent.push(bytes);
    },
    onMessage(listener) {
      listeners.push(listener);
    },
    closed: new Promise(() => {}),
    close() {},
  };
  return {
    connection,
    sent,
    receive(bytes: Uint8Array) {
      for (const listener of listeners) listener(bytes);
    },
  };
}

afterEach(() => {
  vi.useRealTimers();
});

const TIMING = { timeoutMs: 1_000, resendMs: 300 };

describe("ping", () => {
  it("sends the protocol's Ping and resolves with the round-trip time of its Pong", async () => {
    const bridge = await loadBridge(wasm);
    const far = fakeConnection();
    let clock = 100;
    const pinging = ping(far.connection, bridge, 42, TIMING, () => clock);
    // Postcard: variant 0, then the nonce as a varint (crates/protocol/tests).
    expect(far.sent).toEqual([new Uint8Array([0, 42])]);
    clock = 112;
    far.receive(new Uint8Array([0, 42]));
    await expect(pinging).resolves.toBe(12);
  });

  it("ignores other messages and pongs for other nonces", async () => {
    vi.useFakeTimers();
    const bridge = await loadBridge(wasm);
    const far = fakeConnection();
    const pinging = ping(far.connection, bridge, 7, TIMING);
    far.receive(new Uint8Array([0xff, 0xff]));
    far.receive(bridge.encodeClientMessage({ ping: { nonce: 7 } }));
    far.receive(new Uint8Array([0, 8]));
    await vi.advanceTimersByTimeAsync(999);
    far.receive(new Uint8Array([0, 7]));
    await expect(pinging).resolves.toBeGreaterThanOrEqual(0);
  });

  it("sends the Ping again every resend interval until the Pong arrives, then stops", async () => {
    vi.useFakeTimers();
    const bridge = await loadBridge(wasm);
    const far = fakeConnection();
    const pinging = ping(far.connection, bridge, 3, TIMING);
    await vi.advanceTimersByTimeAsync(650);
    expect(far.sent).toHaveLength(3);
    far.receive(new Uint8Array([0, 3]));
    await pinging;
    await vi.advanceTimersByTimeAsync(2_000);
    expect(far.sent).toHaveLength(3);
  });

  it("rejects when no pong arrives in time", async () => {
    vi.useFakeTimers();
    const bridge = await loadBridge(wasm);
    const far = fakeConnection();
    const rejected = expect(ping(far.connection, bridge, 1, TIMING)).rejects.toThrow(
      /no pong within 1000 ms/,
    );
    await vi.advanceTimersByTimeAsync(1_000);
    await rejected;
  });
});
