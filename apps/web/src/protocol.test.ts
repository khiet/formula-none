import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { loadBridge } from "./bridge";
import type { ClientMessage, ServerMessage } from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";

// The fixtures are the sample messages the Rust encoder produced, as bytes
// next to their JSON (crates/protocol/src/samples.rs). The real wasm build
// decodes the bytes into values of the generated types and encodes the
// JSON back into the same bytes, so the generated types describe exactly
// what travels the wire (ADR 0004).
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));

interface Fixture {
  readonly name: string;
  readonly bytes: number[];
  readonly message: unknown;
}
// JSON holds the one u64 field, a seed, as a number too wide for a double,
// so it is read from the source text as the bigint the generated type says.
const fixtures = JSON.parse(
  readFileSync(new URL("./generated/protocol/fixtures.json", import.meta.url), "utf8"),
  (key: string, value: unknown, context?: { source?: string }) =>
    key === "seed" ? BigInt(context?.source ?? String(value)) : value,
) as { readonly client: Fixture[]; readonly server: Fixture[] };

/** The name serde gives a message's variant: a unit variant is its name, any other is the one key. */
type Kind<M> = M extends string ? M : keyof M;

// A record keyed by every variant of the generated union: a variant added in
// Rust fails to compile here until it is listed, and the first test then
// wants its fixture.
const CLIENT_KINDS = {
  ping: true,
  join: true,
  input: true,
  ready: true,
  start: true,
  rematch: true,
  leave: true,
} satisfies Record<Kind<ClientMessage>, true>;
const SERVER_KINDS = {
  pong: true,
  joined: true,
  refused: true,
  room: true,
  countdown: true,
  snapshot: true,
  events: true,
  results: true,
} satisfies Record<Kind<ServerMessage>, true>;

/**
 * JSON carries an f32 as its shortest decimal, which reads as the nearest
 * double, while the bridge reports the f32 itself widened to a double. Put
 * the JSON side in the bridge's form before comparing; an integer is the
 * same in both.
 */
function asBridgeValue(value: unknown): unknown {
  if (typeof value === "number") return Number.isInteger(value) ? value : Math.fround(value);
  if (Array.isArray(value)) return value.map(asBridgeValue);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, asBridgeValue(entry)]),
    );
  }
  return value;
}

describe("generated protocol types", () => {
  it("cover every client and server message type with a fixture", () => {
    expect(fixtures.client.map((fixture) => fixture.name).sort()).toEqual(
      Object.keys(CLIENT_KINDS).sort(),
    );
    expect(fixtures.server.map((fixture) => fixture.name).sort()).toEqual(
      Object.keys(SERVER_KINDS).sort(),
    );
  });

  it("carry the protocol version the bridge was built with", async () => {
    const bridge = await loadBridge(wasm);
    expect(PROTOCOL_VERSION).toBe(bridge.protocolVersion);
  });

  describe.each(fixtures.client)("client message $name", ({ bytes, message }) => {
    it("decodes from the bytes the Rust encoder produced", async () => {
      const bridge = await loadBridge(wasm);
      const decoded: ClientMessage = bridge.decodeClientMessage(new Uint8Array(bytes));
      expect(decoded).toEqual(asBridgeValue(message));
    });

    it("encodes from a value of the generated type to the same bytes", async () => {
      const bridge = await loadBridge(wasm);
      const encoded = bridge.encodeClientMessage(message as ClientMessage);
      expect(Array.from(encoded)).toEqual(bytes);
    });
  });

  describe.each(fixtures.server)("server message $name", ({ bytes, message }) => {
    it("decodes from the bytes the Rust encoder produced", async () => {
      const bridge = await loadBridge(wasm);
      const decoded: ServerMessage = bridge.decodeServerMessage(new Uint8Array(bytes));
      expect(decoded).toEqual(asBridgeValue(message));
    });

    it("encodes from a value of the generated type to the same bytes", async () => {
      const bridge = await loadBridge(wasm);
      const encoded = bridge.encodeServerMessage(message as ServerMessage);
      expect(Array.from(encoded)).toEqual(bytes);
    });
  });

  it("read as the shapes the client code narrows on", async () => {
    // The generated unions narrow by key; this is the use every Scene makes
    // of them, checked once against bytes the server would send.
    const bridge = await loadBridge(wasm);
    const snapshot = fixtures.server.find((fixture) => fixture.name === "snapshot");
    const decoded = bridge.decodeServerMessage(new Uint8Array(snapshot?.bytes ?? []));
    if (!("snapshot" in decoded)) {
      throw new Error("not a snapshot");
    }
    const kart = decoded.snapshot.karts[1];
    expect(kart?.drift?.direction).toBe("left");
    expect(kart?.heldItem).toBeNull();
    expect(decoded.snapshot.events.map((event) => event.kind)).toContainEqual({
      hit: { kart: 6, by: 1 },
    });
  });

  it("rejects bytes that are not a message, with the protocol's reason", async () => {
    const bridge = await loadBridge(wasm);
    expect(() => bridge.decodeServerMessage(new Uint8Array([0xff, 0xff]))).toThrow(
      /malformed message/,
    );
    expect(() => bridge.encodeClientMessage({ ping: { nonce: -1 } })).toThrow();
  });
});
