import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Connection } from "./connection";
import { type FakeConnection, fakeConnection, jsonCodec, TOKEN } from "./fake-connection";
import type { Room, ServerMessage } from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";
import { keepConnected } from "./reconnect";
import { joinRoom } from "./room-client";

const TIMING = { silenceMs: 2_000, retryMs: 1_000 };
const ROOM: Room = {
  code: "BXQF",
  state: "racing",
  players: [{ slot: 0, nickname: "ada", livery: 0, state: "racing", creator: true }],
};
const JOINED: ServerMessage = { joined: { slot: 0, room: ROOM, token: TOKEN } };
const SNAPSHOT: ServerMessage = {
  snapshot: { tick: 1, acknowledged: [0], karts: [], events: [], hazards: [], projectiles: [] },
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
      { rejoin: { version: PROTOCOL_VERSION, room: "BXQF", token: TOKEN } },
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

  it("does not take its own stall for the server's silence", async () => {
    const { first, connect, banner } = await racing();
    await vi.advanceTimersByTimeAsync(1_000);
    first.receive(SNAPSHOT);
    // The page is frozen past the silence: no check runs and nothing is read.
    vi.setSystemTime(Date.now() + 3_000);
    // The overdue check runs before the messages that queued behind the stall.
    await vi.advanceTimersByTimeAsync(TIMING.silenceMs / 8);
    // Then the race goes on as before.
    for (let tick = 0; tick < 5; tick++) {
      first.receive(SNAPSHOT);
      await vi.advanceTimersByTimeAsync(TIMING.silenceMs / 2);
    }
    expect(connect).not.toHaveBeenCalled();
    expect(banner).toEqual([]);
    expect(first.closedByClient).toBe(false);
  });

  it("still reconnects once a stall is over and the server stays silent", async () => {
    const { connect, banner } = await racing();
    vi.setSystemTime(Date.now() + 3_000);
    // The overdue check leaves the call to the next one, in case the
    // server's messages are still queued behind the stall.
    await vi.advanceTimersByTimeAsync(TIMING.silenceMs / 8);
    expect(connect).not.toHaveBeenCalled();
    // Nothing came: the silence was the server's.
    await vi.advanceTimersByTimeAsync(TIMING.silenceMs / 8);
    expect(connect).toHaveBeenCalledTimes(1);
    expect(banner).toEqual([true]);
  });

  it("catches a silent server when every check runs late", async () => {
    const { first, connect, banner } = await racing();
    // A client this slow misses every other check, yet its messages keep it quiet.
    const slow = async () => {
      vi.setSystemTime(Date.now() + TIMING.silenceMs / 8);
      await vi.advanceTimersByTimeAsync(TIMING.silenceMs / 8);
    };
    for (let tick = 0; tick < 8; tick++) {
      first.receive(SNAPSHOT);
      await slow();
    }
    expect(connect).not.toHaveBeenCalled();
    // Once the server stops, the silence is caught within two checks of the tuned time.
    for (let tick = 0; tick < 5; tick++) await slow();
    expect(connect).toHaveBeenCalledTimes(1);
    expect(banner).toEqual([true]);
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
