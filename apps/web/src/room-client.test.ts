import { describe, expect, it } from "vitest";
import { fakeConnection, jsonCodec, TOKEN } from "./fake-connection";
import type { Event, KartSnapshot, Room, Snapshot } from "./generated/protocol";
import { PROTOCOL_VERSION } from "./generated/protocol";
import { IDLE_INPUT } from "./input";
import { joinRoom, quantiseInput, RefusedError, refusalMessage, rejoinRoom } from "./room-client";

const codec = jsonCodec;

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
    connection.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
    const client = await joining;
    expect(client.slot).toBe(1);
    expect(client.room).toEqual(ROOM);
    expect(client.token).toBe(TOKEN);
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
    connection.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
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

    let countdown: { seed: bigint; track: string; grid: readonly number[] } | undefined;
    client.onCountdown((start) => {
      countdown = start;
    });
    // JSON carries the seed as a number here; the real codec gives a bigint.
    connection.receive({
      countdown: { seed: 42 as unknown as bigint, track: "brands-patch", grid: [0, 1] },
    });
    expect(countdown).toEqual({ seed: 42n, track: "brands-patch", grid: [0, 1] });

    let ended = false;
    client.closed.then(() => {
      ended = true;
    });
    connection.end();
    await client.closed;
    expect(ended).toBe(true);
  });

  it("sends Ready, Start, Rematch and Leave on the reliable lane, and Leave closes the connection", async () => {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "BXQF", nickname: "ada" });
    connection.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
    const client = await joining;
    connection.sent.length = 0;
    connection.lanes.length = 0;

    client.ready(true);
    client.ready(false);
    client.start();
    client.rematch();
    client.leave();
    expect(connection.sent).toEqual([
      { ready: { ready: true } },
      { ready: { ready: false } },
      "start",
      "rematch",
      "leave",
    ]);
    expect(connection.lanes).toEqual(Array(5).fill("reliable"));
    expect(connection.closedByClient).toBe(true);
  });

  it("stops a listener that unsubscribed", async () => {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "BXQF", nickname: "ada" });
    connection.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
    const client = await joining;
    const seen: Room[] = [];
    const stop = client.onRoom((room) => seen.push(room));
    connection.receive({ room: ROOM });
    stop();
    stop();
    connection.receive({ room: ROOM });
    expect(seen).toHaveLength(1);
  });
});

const KART: KartSnapshot = {
  position: [0, 0.3, 0],
  rotation: [0, 0, 0, 1],
  velocity: [0, 0, 0],
  bump: null,
  drift: null,
  racePosition: 1,
  lap: 1,
  checkpoint: 0,
  splineProgress: 0,
  wrongWay: false,
  heldItem: null,
  spinOutS: null,
  start: "clean",
  stallS: null,
  boost: null,
};

function snapshot(tick: number, events: Event[] = []): Snapshot {
  return { tick, acknowledged: [0, 0], karts: [KART, KART], events, hazards: [], projectiles: [] };
}

function lapEvent(id: number, kart: number): Event {
  return { id, kind: { lapCompleted: { kart, lap: 1, lapTimeS: 40 } } };
}

describe("the race side of the room client", () => {
  async function racing() {
    const connection = fakeConnection();
    const joining = joinRoom(connection, codec, { code: "BXQF", nickname: "ada" });
    connection.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
    const client = await joining;
    connection.receive({
      countdown: { seed: 1 as unknown as bigint, track: "brands-patch", grid: [0, 1] },
    });
    connection.sent.length = 0;
    connection.lanes.length = 0;
    return { connection, client };
  }

  it("delivers snapshots in tick order and drops a late or repeated one", async () => {
    const { connection, client } = await racing();
    const ticks: number[] = [];
    client.onSnapshot((received) => ticks.push(received.tick));
    for (const tick of [2, 4, 8, 6, 8, 10]) connection.receive({ snapshot: snapshot(tick) });
    expect(ticks).toEqual([2, 4, 8, 10]);
    // The next race's ticks start over.
    connection.receive({
      countdown: { seed: 2 as unknown as bigint, track: "brands-patch", grid: [0, 1] },
    });
    connection.receive({ snapshot: snapshot(2) });
    expect(ticks).toEqual([2, 4, 8, 10, 2]);
  });

  it("delivers each event once, whichever lane brings it first (ADR 0011)", async () => {
    const { connection, client } = await racing();
    const ids: number[] = [];
    client.onEvent((event) => ids.push(event.id));
    // On the stream first, then repeated by three snapshots.
    connection.receive({ events: [lapEvent(1, 0)] });
    connection.receive({ snapshot: snapshot(2, [lapEvent(1, 0)]) });
    connection.receive({ snapshot: snapshot(4, [lapEvent(1, 0), lapEvent(2, 1)]) });
    // The stream's copy of event 2 arrives after its snapshot repeat.
    connection.receive({ events: [lapEvent(2, 1)] });
    connection.receive({ snapshot: snapshot(6, [lapEvent(1, 0), lapEvent(2, 1)]) });
    // A snapshot that is itself late still brings a new event.
    connection.receive({ snapshot: snapshot(5, [lapEvent(3, 2)]) });
    expect(ids).toEqual([1, 2, 3]);
  });

  it("hands the results on", async () => {
    const { connection, client } = await racing();
    let results: readonly unknown[] | undefined;
    client.onResults((standings) => {
      results = standings;
    });
    const standings = [{ kart: 1, position: 1, finishTimeS: 90.5, bestLapS: 30 }];
    connection.receive({ results: { standings } });
    expect(results).toEqual(standings);
  });

  it("sends inputs on the datagram lane, quantised, under a sequence from 1 that each Countdown restarts", async () => {
    const { connection, client } = await racing();
    expect(client.sendInput({ ...IDLE_INPUT, steer: -1, throttle: 1 })).toBe(1);
    expect(client.sendInput({ ...IDLE_INPUT, steer: 0.5, brake: 0.5, hop: true })).toBe(2);
    expect(connection.lanes).toEqual(["datagram", "datagram"]);
    expect(connection.sent).toEqual([
      {
        input: {
          sequence: 1,
          steer: -127,
          throttle: 255,
          brake: 0,
          hop: false,
          useItem: false,
          holdItem: false,
        },
      },
      {
        input: {
          sequence: 2,
          steer: 64,
          throttle: 0,
          brake: 128,
          hop: true,
          useItem: false,
          holdItem: false,
        },
      },
    ]);
    connection.receive({
      countdown: { seed: 2 as unknown as bigint, track: "brands-patch", grid: [0, 1] },
    });
    expect(client.sendInput(IDLE_INPUT)).toBe(1);
  });
});

describe("taking the seat back (ADR 0012)", () => {
  const SEED = 7 as unknown as bigint;
  const COUNTDOWN = { seed: SEED, track: "brands-patch", grid: [0, 1] };

  async function racing() {
    const first = fakeConnection();
    const joining = joinRoom(first, codec, { code: "BXQF", nickname: "ada" });
    first.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
    const client = await joining;
    first.receive({ countdown: COUNTDOWN });
    return { first, client };
  }

  it("rejoins with the session token and resolves on Joined", async () => {
    const connection = fakeConnection();
    const rejoining = rejoinRoom(connection, codec, { code: "bxqf", token: TOKEN });
    expect(connection.sent).toEqual([
      { rejoin: { version: PROTOCOL_VERSION, room: "BXQF", token: TOKEN } },
    ]);
    connection.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
    const client = await rejoining;
    expect(client.slot).toBe(1);
    expect(client.token).toBe(TOKEN);
  });

  it("rejects a rejoin the room no longer has a seat for", async () => {
    const connection = fakeConnection();
    const rejoining = rejoinRoom(connection, codec, { code: "BXQF", token: TOKEN });
    connection.receive({ refused: "unknownSession" });
    await expect(rejoining).rejects.toMatchObject({ refusal: "unknownSession" });
  });

  it("knows the race under way from its Countdown until its Results", async () => {
    const { first, client } = await racing();
    expect(client.race).toEqual({ ...COUNTDOWN, seed: 7n });
    first.receive({ results: { standings: [] } });
    expect(client.race).toBeUndefined();
  });

  it("resumes over a new connection: same listeners, the old one closed and ignored, the race carried on", async () => {
    const { first, client } = await racing();
    const ticks: number[] = [];
    const countdowns: unknown[] = [];
    client.onSnapshot((received) => ticks.push(received.tick));
    client.onCountdown((countdown) => countdowns.push(countdown));
    first.receive({ snapshot: snapshot(10) });
    expect(client.sendInput(IDLE_INPUT)).toBe(1);

    const second = fakeConnection();
    const resuming = client.resume(second);
    expect(first.closedByClient).toBe(true);
    expect(second.sent).toEqual([
      { rejoin: { version: PROTOCOL_VERSION, room: "BXQF", token: TOKEN } },
    ]);
    second.receive({ joined: { slot: 1, room: ROOM, token: TOKEN } });
    await resuming;
    // The server sends the race under way again; it is not a new one, so
    // the sequence and the tick order run on.
    second.receive({ countdown: COUNTDOWN });
    first.receive({ snapshot: snapshot(20) });
    second.receive({ snapshot: snapshot(8) });
    second.receive({ snapshot: snapshot(30) });
    expect(ticks).toEqual([10, 30]);
    expect(countdowns).toEqual([]);
    expect(client.sendInput(IDLE_INPUT)).toBe(2);
    expect(second.lanes.at(-1)).toBe("datagram");
    // The next race is a new one.
    second.receive({ countdown: { ...COUNTDOWN, seed: 8 as unknown as bigint } });
    expect(countdowns).toHaveLength(1);
  });

  it("rejects a resume the server refuses", async () => {
    const { client } = await racing();
    const second = fakeConnection();
    const resuming = client.resume(second);
    second.receive({ refused: "unknownSession" });
    await expect(resuming).rejects.toBeInstanceOf(RefusedError);
  });

  it("tells each message as heard, and stops on Replaced with the connection closed", async () => {
    const { first, client } = await racing();
    let heard = 0;
    client.onHeard(() => {
      heard += 1;
    });
    first.receive({ snapshot: snapshot(2) });
    first.receive({ room: ROOM });
    expect(heard).toBe(2);
    expect(client.replaced).toBe(false);
    first.receive("replaced");
    expect(client.replaced).toBe(true);
    expect(first.closedByClient).toBe(true);
  });
});

describe("quantiseInput", () => {
  it("clamps each axis to its byte", () => {
    expect(quantiseInput({ ...IDLE_INPUT, steer: 3, throttle: -1, brake: 2 }, 9)).toMatchObject({
      sequence: 9,
      steer: 127,
      throttle: 0,
      brake: 255,
    });
  });
});

describe("refusalMessage", () => {
  it("tells the player what to do for each refusal", () => {
    expect(refusalMessage("unknownRoom")).toBe("No room has that code");
    expect(refusalMessage("roomFull")).toBe("That room is full");
    expect(refusalMessage("roomRacing")).toBe("That room is mid-race; join after the race");
    expect(refusalMessage("unknownSession")).toBe("That race is over; join the room again");
    expect(refusalMessage("sessionInUse")).toBe("You are already in that room in another tab");
    expect(refusalMessage({ versionMismatch: { server: 2 } })).toBe(
      "Your game is out of date: reload the page",
    );
  });
});
