import { describe, expect, it } from "vitest";
import type { MessageCodec } from "./bridge";
import type { Connection } from "./connection";
import type { ClientMessage, Room, ServerMessage } from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";
import { joinRoom, RefusedError, refusalMessage } from "./room-client";

// The wire format is the bridge's; here JSON stands in for it, so the test
// reads the messages the client sends and feeds it the server's replies.
const codec: MessageCodec = {
  encodeClientMessage: (message) => new TextEncoder().encode(JSON.stringify(message)),
  decodeClientMessage: (bytes) => JSON.parse(new TextDecoder().decode(bytes)) as ClientMessage,
  encodeServerMessage: (message) => new TextEncoder().encode(JSON.stringify(message)),
  decodeServerMessage: (bytes) => JSON.parse(new TextDecoder().decode(bytes)) as ServerMessage,
};

interface FakeConnection extends Connection {
  readonly sent: ClientMessage[];
  /** Deliver a message as if from the server. */
  receive(message: ServerMessage): void;
  /** End the connection from the server's side. */
  end(): void;
  readonly closedByClient: boolean;
}

function fakeConnection(): FakeConnection {
  const sent: ClientMessage[] = [];
  const listeners = new Set<(bytes: Uint8Array) => void>();
  let end: () => void = () => {};
  const closed = new Promise<void>((resolve) => {
    end = resolve;
  });
  let closedByClient = false;
  return {
    transport: "websocket",
    sent,
    send(bytes) {
      sent.push(codec.decodeClientMessage(bytes));
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
      const bytes = codec.encodeServerMessage(message);
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

const ROOM: Room = {
  code: "BXQF",
  state: "open",
  players: [
    { slot: 0, nickname: "khiet", livery: 0, state: "unready", creator: true },
    { slot: 1, nickname: "ada", livery: 1, state: "unready", creator: false },
  ],
};

describe("joinRoom", () => {
  it("sends the Join with this build's version and resolves on Joined", async () => {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "bxqf", nickname: "ada" });
    expect(connection.sent).toEqual([
      { join: { version: PROTOCOL_VERSION, room: "BXQF", nickname: "ada" } },
    ]);
    connection.receive({ joined: { slot: 1, room: ROOM } });
    const client = await joining;
    expect(client.slot).toBe(1);
    expect(client.room).toEqual(ROOM);
  });

  it("rejects with the refusal and its message when refused", async () => {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "ZZZZ", nickname: "ada" });
    connection.receive({ refused: "unknownRoom" });
    await expect(joining).rejects.toBeInstanceOf(RefusedError);
    await expect(joining).rejects.toMatchObject({
      refusal: "unknownRoom",
      message: refusalMessage("unknownRoom"),
    });
  });

  it("rejects when the connection ends before an answer", async () => {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "BXQF", nickname: "ada" });
    connection.end();
    await expect(joining).rejects.toThrow(/connection closed/);
  });

  it("follows the room through its updates, hands over Countdown, and reports the end", async () => {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "BXQF", nickname: "ada" });
    connection.receive({ joined: { slot: 1, room: ROOM } });
    const client = await joining;

    const seen: Room[] = [];
    client.onRoom((room) => seen.push(room));
    const ready: Room = {
      ...ROOM,
      players: ROOM.players.map((player) => ({ ...player, state: "ready" })),
    };
    connection.receive({ room: ready });
    expect(seen).toEqual([ready]);
    expect(client.room).toEqual(ready);

    let countdown: { seed: bigint; track: string } | undefined;
    client.onCountdown((start) => {
      countdown = start;
    });
    // JSON carries the seed as a number here; the real codec gives a bigint.
    connection.receive({ countdown: { seed: 42 as unknown as bigint, track: "brands-patch" } });
    expect(countdown).toEqual({ seed: 42n, track: "brands-patch" });

    let ended = false;
    client.closed.then(() => {
      ended = true;
    });
    connection.end();
    await client.closed;
    expect(ended).toBe(true);
  });

  it("sends Ready, Start and Leave, and Leave closes the connection", async () => {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "BXQF", nickname: "ada" });
    connection.receive({ joined: { slot: 1, room: ROOM } });
    const client = await joining;
    connection.sent.length = 0;

    client.ready(true);
    client.ready(false);
    client.start();
    client.leave();
    expect(connection.sent).toEqual([
      { ready: { ready: true } },
      { ready: { ready: false } },
      "start",
      "leave",
    ]);
    expect(connection.closedByClient).toBe(true);
  });
});

describe("refusalMessage", () => {
  it("tells the player what to do for each refusal", () => {
    expect(refusalMessage("unknownRoom")).toBe("No room has that code");
    expect(refusalMessage("roomFull")).toBe("That room is full");
    expect(refusalMessage("roomRacing")).toBe("That room is mid-race; join after the race");
    expect(refusalMessage({ versionMismatch: { server: 2 } })).toBe(
      "Your game is out of date: reload the page",
    );
  });
});
