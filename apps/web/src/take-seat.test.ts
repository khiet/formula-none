import { describe, expect, it } from "vitest";
import { type FakeConnection, fakeConnection, jsonCodec, TOKEN } from "./fake-connection";
import type { Room } from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";
import { loadSession, saveSession } from "./room-session";
import { takeSeat } from "./take-seat";

/** The slice of Storage the session uses, in memory. */
function storage(initial: Record<string, string> = {}): Storage {
  const items = new Map(Object.entries(initial));
  return {
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
    clear: () => items.clear(),
    key: (index) => [...items.keys()][index] ?? null,
    get length() {
      return items.size;
    },
  };
}

const ROOM: Room = {
  code: "BXQF",
  track: "goldstone",
  botLevel: "basic",
  tyres: "off",
  state: "open",
  players: [
    { slot: 0, nickname: "khiet", livery: 0, state: "unready", creator: true },
    { slot: 1, nickname: "ada", livery: 1, state: "unready", creator: false },
  ],
};
const DEALT = "fedcba9876543210fedcba9876543210";
/** The code as a player might type it; the server is sent "BXQF". */
const REQUEST = { code: "bxqf", nickname: "ada" };

/**
 * A server reached over a new fake connection per attempt: the nth of `answers`
 * replies to the nth connection's first message, as the server would.
 */
function server(...answers: ((connection: FakeConnection) => void)[]) {
  const connections: FakeConnection[] = [];
  return {
    connections,
    async connect(): Promise<FakeConnection> {
      const connection = fakeConnection();
      const answer = answers[connections.length];
      connections.push(connection);
      // The client listens before it sends, so the answer goes out on its first message.
      const send = connection.send.bind(connection);
      connection.send = (bytes, lane) => {
        send(bytes, lane);
        if (connection.sent.length === 1) answer?.(connection);
      };
      return connection;
    },
  };
}

const joined = (token: string) => (connection: FakeConnection) =>
  connection.receive({ joined: { slot: 1, room: ROOM, token } });

describe("taking a seat in a room", () => {
  it("joins as a new player when no token is kept, and keeps the token dealt", async () => {
    const store = storage();
    const reached = server(joined(DEALT));
    const seat = await takeSeat({ storage: store, codec: jsonCodec, ...reached }, REQUEST);
    expect(reached.connections[0]?.sent).toEqual([
      { join: { version: PROTOCOL_VERSION, room: "BXQF", nickname: "ada" } },
    ]);
    expect(seat.kind).toBe("room");
    expect(seat.client.slot).toBe(1);
    expect(loadSession(store, "BXQF")).toBe(DEALT);
  });

  it("rejoins with the token kept for the room", async () => {
    const store = storage();
    saveSession(store, { code: "BXQF", token: TOKEN });
    const reached = server(joined(TOKEN));
    const seat = await takeSeat({ storage: store, codec: jsonCodec, ...reached }, REQUEST);
    expect(reached.connections).toHaveLength(1);
    expect(reached.connections[0]?.sent).toEqual([
      { rejoin: { version: PROTOCOL_VERSION, room: "BXQF", token: TOKEN } },
    ]);
    expect(seat).toMatchObject({ kind: "room", client: { token: TOKEN } });
    expect(loadSession(store, "BXQF")).toBe(TOKEN);
  });

  it("forgets a token the room refuses and joins over a fresh connection", async () => {
    const store = storage();
    saveSession(store, { code: "BXQF", token: TOKEN });
    const reached = server(
      (connection) => connection.receive({ refused: "unknownSession" }),
      joined(DEALT),
    );
    const seat = await takeSeat({ storage: store, codec: jsonCodec, ...reached }, REQUEST);
    expect(reached.connections).toHaveLength(2);
    expect(reached.connections[1]?.sent).toEqual([
      { join: { version: PROTOCOL_VERSION, room: "BXQF", nickname: "ada" } },
    ]);
    expect(seat.client.token).toBe(DEALT);
    expect(loadSession(store, "BXQF")).toBe(DEALT);
  });

  it("passes on a rejoin that fails other than by refusal, keeping the token", async () => {
    const store = storage();
    saveSession(store, { code: "BXQF", token: TOKEN });
    const reached = server((connection) => connection.end());
    await expect(
      takeSeat({ storage: store, codec: jsonCodec, ...reached }, REQUEST),
    ).rejects.toThrow(/connection closed/);
    expect(reached.connections).toHaveLength(1);
    expect(loadSession(store, "BXQF")).toBe(TOKEN);
  });

  it("passes on a refused join, the refused token already forgotten", async () => {
    const store = storage();
    saveSession(store, { code: "BXQF", token: TOKEN });
    const reached = server(
      (connection) => connection.receive({ refused: "unknownSession" }),
      (connection) => connection.receive({ refused: "unknownRoom" }),
    );
    await expect(
      takeSeat({ storage: store, codec: jsonCodec, ...reached }, REQUEST),
    ).rejects.toMatchObject({ refusal: "unknownRoom" });
    expect(loadSession(store, "BXQF")).toBeUndefined();
  });

  it("goes straight to the race a rejoin finds under way", async () => {
    const store = storage();
    saveSession(store, { code: "BXQF", token: TOKEN });
    const reached = server((connection) => {
      joined(TOKEN)(connection);
      // JSON carries the seed as a number here; the real codec gives a bigint.
      connection.receive({
        countdown: {
          seed: 7 as unknown as bigint,
          track: "bell-deer",
          botLevel: "basic",
          tyres: "off",
          grid: [0, 1],
        },
      });
    });
    const seat = await takeSeat({ storage: store, codec: jsonCodec, ...reached }, REQUEST);
    expect(seat).toMatchObject({
      kind: "race",
      countdown: { seed: 7n, track: "bell-deer", botLevel: "basic", grid: [0, 1] },
    });
  });
});
