import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Connection } from "./connection";
import { type FakeConnection, fakeConnection, jsonCodec } from "./fake-connection";
import type { Room, ServerMessage } from "./generated/protocol";
import { keepConnected } from "./reconnect";
import { joinRoom } from "./room-client";

const TIMING = { silenceMs: 2_000, retryMs: 1_000 };
const TOKEN = "0123456789abcdef0123456789abcdef";
const ROOM: Room = {
  code: "BXQF",
  state: "racing",
  players: [{ slot: 0, nickname: "ada", livery: 0, state: "racing", creator: true }],
};
const JOINED: ServerMessage = { joined: { slot: 0, room: ROOM, token: TOKEN } };
const SNAPSHOT: ServerMessage = {
  snapshot: { tick: 1, acknowledged: [0], karts: [], events: [] },
};

/** A racing room client, and the connections a reconnect is handed, in order. */
async function racing(refuseConnects = 0) {
  const first = fakeConnection();
  const joining = joinRoom(first, jsonCodec, { code: "BXQF", nickname: "ada" });
  first.receive(JOINED);
  const client = await joining;
  const opened: FakeConnection[] = [];
  let refusals = refuseConnects;
  const connect = vi.fn(async (): Promise<Connection> => {
    if (refusals > 0) {
      refusals -= 1;
      throw new Error("no route to the server");
    }
    const connection = fakeConnection();
    opened.push(connection);
    return connection;
  });
  const link = keepConnected(client, { timing: TIMING, connect, now: () => Date.now() });
  const banner: boolean[] = [];
  link.onLost((lost) => banner.push(lost));
  return { first, client, connect, opened, link, banner };
}

describe("keeping a race connected (ADR 0012)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("raises the banner after the tuned silence, rejoins on a new connection, and lowers it", async () => {
    const { first, connect, opened, banner } = await racing();
    // Messages keep it quiet however long the race runs.
    for (let tick = 0; tick < 10; tick++) {
      await vi.advanceTimersByTimeAsync(1_000);
      first.receive(SNAPSHOT);
    }
    expect(banner).toEqual([]);

    await vi.advanceTimersByTimeAsync(TIMING.silenceMs + 500);
    expect(banner).toEqual([true]);
    expect(connect).toHaveBeenCalledTimes(1);
    expect(first.closedByClient).toBe(true);
    const second = opened[0] as FakeConnection;
    expect(second.sent).toEqual([
      { rejoin: { version: expect.any(Number), room: "BXQF", token: TOKEN } },
    ]);

    second.receive(JOINED);
    await vi.advanceTimersByTimeAsync(0);
    expect(banner).toEqual([true, false]);
    // Heard from on the new connection, it stays down.
    for (let tick = 0; tick < 5; tick++) {
      await vi.advanceTimersByTimeAsync(1_000);
      second.receive(SNAPSHOT);
    }
    expect(banner).toEqual([true, false]);
  });

  it("reconnects at once when the connection ends", async () => {
    const { first, connect, banner } = await racing();
    first.end();
    await vi.advanceTimersByTimeAsync(0);
    expect(banner).toEqual([true]);
    expect(connect).toHaveBeenCalledTimes(1);
  });

  it("tries again at the tuned interval until the server is reached", async () => {
    const { first, connect, opened, banner } = await racing(2);
    first.end();
    await vi.advanceTimersByTimeAsync(0);
    expect(connect).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(TIMING.retryMs);
    expect(connect).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(TIMING.retryMs);
    expect(connect).toHaveBeenCalledTimes(3);
    (opened[0] as FakeConnection).receive(JOINED);
    await vi.advanceTimersByTimeAsync(0);
    expect(banner).toEqual([true, false]);
  });

  it("closes a rejoin the server never answers and tries again", async () => {
    const { first, connect, opened } = await racing();
    first.end();
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(TIMING.silenceMs);
    expect(opened[0]?.closedByClient).toBe(true);
    await vi.advanceTimersByTimeAsync(TIMING.retryMs);
    expect(connect).toHaveBeenCalledTimes(2);
  });

  it("gives up when the server refuses the rejoin", async () => {
    const { first, opened, link, banner } = await racing();
    first.end();
    await vi.advanceTimersByTimeAsync(0);
    (opened[0] as FakeConnection).receive({ refused: "unknownSession" });
    await expect(link.gone).resolves.toBe("refused");
    await vi.advanceTimersByTimeAsync(10 * TIMING.retryMs);
    expect(opened).toHaveLength(1);
    expect(banner).toEqual([true]);
  });

  it("gives up without reconnecting when another connection took the seat", async () => {
    const { first, connect, link, banner } = await racing();
    first.receive("replaced");
    await expect(link.gone).resolves.toBe("replaced");
    await vi.advanceTimersByTimeAsync(10 * TIMING.silenceMs);
    expect(connect).not.toHaveBeenCalled();
    expect(banner).toEqual([]);
  });

  it("does nothing more once disposed", async () => {
    const { first, connect, link } = await racing();
    link.dispose();
    first.end();
    await vi.advanceTimersByTimeAsync(10 * TIMING.silenceMs);
    expect(connect).not.toHaveBeenCalled();
  });
});
