import { describe, expect, it } from "vitest";
import type { Bridge, KartInput, KartTransform, Race } from "./bridge";
import type { Event, FinalStanding, KartSnapshot, Snapshot } from "./generated/protocol";
import { IDLE_INPUT } from "./input";
import type { Countdown, RoomClient } from "./room-client";
import { createServerRace, speedAlongFacing } from "./server-race";

// The run reads the server's snapshots and events through the room client;
// the bridge here is only what the run asks of it: the tuning, a race it
// never steps, for the grid and the surface under a point.

const TICKS_PER_SECOND = 60;
const COUNTDOWN_S = 3;
const KARTS = 8;

const GRID_TRANSFORM: KartTransform = {
  position: [1, 0.3, 2],
  rotation: [0, 0, 0, 1],
  speed: 0,
  surface: "asphalt",
  drift: null,
};

const bridge = {
  ticksPerSecond: TICKS_PER_SECOND,
  countdownSeconds: COUNTDOWN_S,
  kartsPerRace: KARTS,
  lapsPerRace: 3,
  miniTurboTier: (chargeSeconds: number) =>
    chargeSeconds >= 1.6 ? "orange" : chargeSeconds >= 0.8 ? "blue" : "none",
  createRace: () =>
    ({
      kartTransforms: () => Array<KartTransform>(KARTS).fill(GRID_TRANSFORM),
      surfaceAt: ([x]: readonly [number, number, number]) => (x > 100 ? "out of world" : "asphalt"),
    }) as unknown as Race,
} as unknown as Bridge;

interface FakeClient extends RoomClient {
  readonly inputs: KartInput[];
  snapshot(snapshot: Snapshot): void;
  event(event: Event): void;
  results(standings: FinalStanding[]): void;
}

function fakeClient(): FakeClient {
  const inputs: KartInput[] = [];
  const snapshots = new Set<(snapshot: Snapshot) => void>();
  const events = new Set<(event: Event) => void>();
  const results = new Set<(standings: readonly FinalStanding[]) => void>();
  const subscribe =
    <L>(listeners: Set<L>) =>
    (listener: L) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    };
  return {
    slot: 1,
    room: { code: "BXQF", state: "racing", players: [] },
    closed: new Promise(() => {}),
    inputs,
    onRoom: () => () => {},
    onCountdown: () => () => {},
    onSnapshot: subscribe(snapshots),
    onEvent: subscribe(events),
    onResults: subscribe(results),
    ready() {},
    start() {},
    rematch() {},
    leave() {},
    sendInput(input) {
      inputs.push(input);
      return inputs.length;
    },
    snapshot(snapshot) {
      for (const listener of snapshots) listener(snapshot);
    },
    event(event) {
      for (const listener of events) listener(event);
    },
    results(standings) {
      for (const listener of results) listener(standings);
    },
  };
}

const COUNTDOWN: Countdown = { seed: 7n, track: "brands-patch", grid: [0, 1] };

const KART: KartSnapshot = {
  position: [10, 0.3, -20],
  rotation: [0, 0, 0, 1],
  velocity: [0, 0, -12],
  drift: null,
  racePosition: 3,
  lap: 2,
  checkpoint: 4,
  splineProgress: 321,
  wrongWay: false,
  heldItem: null,
  spinOutS: null,
};

function snapshot(tick: number, karts: Partial<KartSnapshot>[] = []): Snapshot {
  return {
    tick,
    acknowledged: Array<number>(KARTS).fill(0),
    karts: Array.from({ length: KARTS }, (_, kart) => ({ ...KART, ...karts[kart] })),
    events: [],
  };
}

function lap(id: number, kart: number, lapTimeS: number): Event {
  return { id, kind: { lapCompleted: { kart, lap: 1, lapTimeS } } };
}

describe("createServerRace", () => {
  it("puts the local player on the kart their slot has on the grid and the bots behind the humans", () => {
    const run = createServerRace(bridge, fakeClient(), COUNTDOWN, 1, "{}");
    expect(run.localKart).toBe(1);
    expect(run.humanCount).toBe(2);
    expect(() => createServerRace(bridge, fakeClient(), COUNTDOWN, 5, "{}")).toThrow(/grid/);
  });

  it("stands the karts on the grid in the countdown until the first snapshot", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    expect(run.kartTransforms()).toHaveLength(KARTS);
    expect(run.kartTransforms()[0]).toEqual(GRID_TRANSFORM);
    expect(run.phase()).toEqual({ kind: "countdown", remainingSeconds: COUNTDOWN_S });
    expect(run.standings()[0]).toMatchObject({ position: 1, lap: 1, lapTimeSeconds: 0 });
    client.snapshot(snapshot(60));
    expect(run.phase()).toEqual({ kind: "countdown", remainingSeconds: 2 });
  });

  it("sends each tick's input to the server", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    const input = { ...IDLE_INPUT, throttle: 1 };
    run.tick(input, 1 / TICKS_PER_SECOND);
    expect(client.inputs).toEqual([input]);
  });

  it("draws every kart from the newest snapshot, with the sim's surface and the drift's tier", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    client.snapshot(
      snapshot(200, [
        {},
        {
          position: [500, 0, 0],
          drift: { direction: "left", chargeS: 1.0 },
          velocity: [0, 0, 5],
        },
      ]),
    );
    const [first, second] = run.kartTransforms();
    expect(first).toEqual({
      position: [10, 0.3, -20],
      rotation: [0, 0, 0, 1],
      speed: 12,
      surface: "asphalt",
      drift: null,
    });
    expect(second).toMatchObject({
      surface: "out of world",
      speed: -5,
      drift: { direction: "left", chargeSeconds: 1.0, miniTurbo: "blue" },
    });
  });

  it("reads the race clock from the tick and the lap clocks from the events", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    client.snapshot(snapshot(180 + 60 * 50, [{ racePosition: 2, lap: 2, wrongWay: true }]));
    expect(run.phase()).toEqual({ kind: "racing", elapsedSeconds: 50, graceLeftSeconds: null });
    client.event(lap(1, 0, 41.5));
    const standing = run.standings()[0];
    expect(standing).toMatchObject({
      position: 2,
      lap: 2,
      checkpoint: 4,
      splineProgress: 321,
      bestLapSeconds: 41.5,
      wrongWay: true,
      finishTimeSeconds: null,
    });
    expect(standing?.lapTimeSeconds).toBeCloseTo(8.5);
    client.event(lap(2, 0, 40));
    expect(run.standings()[0]?.bestLapSeconds).toBe(40);
  });

  it("finishes on the results, whose standings replace the snapshot's and stop the lap clock", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    client.snapshot(snapshot(180 + 60 * 130));
    client.event(lap(1, 0, 41.5));
    client.event(lap(2, 0, 40));
    client.event({ id: 3, kind: { lapCompleted: { kart: 0, lap: 3, lapTimeS: 42 } } });
    client.event({ id: 4, kind: { finished: { kart: 0, position: 1, finishTimeS: 123.5 } } });
    expect(run.phase().kind).toBe("racing");
    expect(run.standings()[0]).toMatchObject({ finishTimeSeconds: 123.5, lapTimeSeconds: 42 });
    client.results([
      { kart: 0, position: 1, finishTimeS: 123.5, bestLapS: 40 },
      { kart: 1, position: 2, finishTimeS: null, bestLapS: null },
    ]);
    expect(run.phase()).toEqual({ kind: "finished" });
    expect(run.standings()[1]).toMatchObject({
      position: 2,
      finishTimeSeconds: null,
      bestLapSeconds: null,
    });
    // Disposed, the run stops following the client.
    run.dispose();
    client.snapshot(snapshot(180 + 60 * 131, [{ racePosition: 5 }]));
    expect(run.standings()[0]?.position).toBe(1);
  });
});

describe("speedAlongFacing", () => {
  it("is the velocity along -Z for an unturned kart, and along the facing once turned", () => {
    expect(speedAlongFacing([0, 0, -12], [0, 0, 0, 1])).toBeCloseTo(12);
    expect(speedAlongFacing([0, 0, 12], [0, 0, 0, 1])).toBeCloseTo(-12);
    // A quarter turn about +Y faces -X.
    const half = Math.SQRT1_2;
    expect(speedAlongFacing([-9, 0, 0], [0, half, 0, half])).toBeCloseTo(9);
    expect(speedAlongFacing([0, 0, -9], [0, half, 0, half])).toBeCloseTo(0);
  });
});
