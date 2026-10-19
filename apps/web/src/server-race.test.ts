import { beforeEach, describe, expect, it } from "vitest";
import type { Bridge, KartInput, KartTransform, Phase, Prediction, Race } from "./bridge";
import type { Event, FinalStanding, Input, KartSnapshot, Snapshot } from "./generated/protocol";
import { IDLE_INPUT } from "./input";
import { type Countdown, quantiseInput, type RoomClient } from "./room-client";
import { createServerRace } from "./server-race";

// The run reads the server's snapshots and events through the room client;
// the bridge here is only what the run asks of it: the tuning, a race it
// never steps, for the grid, and the own kart's prediction, which records
// what it is given and reports what the test sets, the other karts'
// interpolation included.

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

const PREDICTED: KartTransform = {
  position: [3, 0.3, -40],
  rotation: [0, 0, 0, 1],
  speed: 20,
  surface: "asphalt",
  drift: null,
};

interface FakePrediction extends Prediction {
  readonly kart: number;
  readonly inputs: Input[];
  readonly snapshots: Snapshot[];
  clock: Phase;
  interpolated: KartTransform[];
}

let predictions: FakePrediction[] = [];

/** The prediction the run under test made. */
function prediction(): FakePrediction {
  const made = predictions.at(-1);
  if (made === undefined) throw new Error("the run made no prediction");
  return made;
}

const bridge = {
  ticksPerSecond: TICKS_PER_SECOND,
  countdownSeconds: COUNTDOWN_S,
  kartsPerRace: KARTS,
  lapsPerRace: 3,
  createRace: () =>
    ({
      kartTransforms: () => Array<KartTransform>(KARTS).fill(GRID_TRANSFORM),
    }) as unknown as Race,
  createPrediction(_seed: bigint, kart: number): Prediction {
    const made: FakePrediction = {
      kart,
      inputs: [],
      snapshots: [],
      clock: { kind: "countdown", remainingSeconds: COUNTDOWN_S },
      interpolated: [],
      tick(input) {
        this.inputs.push(input);
      },
      reconcile(snapshot) {
        this.snapshots.push(snapshot);
        return 0;
      },
      kartTransform: () => PREDICTED,
      interpolatedKartTransforms() {
        return [...this.interpolated];
      },
      phase() {
        return this.clock;
      },
      correctionsPerSecond: () => 0.5,
    };
    predictions.push(made);
    return made;
  },
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
  beforeEach(() => {
    predictions = [];
  });

  it("puts the local player on the kart their slot has on the grid and the bots behind the humans", () => {
    const run = createServerRace(bridge, fakeClient(), COUNTDOWN, 1, "{}");
    expect(run.localKart).toBe(1);
    expect(run.humanCount).toBe(2);
    expect(prediction().kart).toBe(1);
    expect(() => createServerRace(bridge, fakeClient(), COUNTDOWN, 5, "{}")).toThrow(/grid/);
  });

  it("stands the other karts on the grid until the first snapshot, the own kart where it is predicted", () => {
    const run = createServerRace(bridge, fakeClient(), COUNTDOWN, 0, "{}");
    const transforms = run.kartTransforms();
    expect(transforms).toHaveLength(KARTS);
    expect(transforms[0]).toEqual(PREDICTED);
    expect(transforms[1]).toEqual(GRID_TRANSFORM);
    expect(run.phase()).toEqual({ kind: "countdown", remainingSeconds: COUNTDOWN_S });
    expect(run.standings()[0]).toMatchObject({ position: 1, lap: 1, lapTimeSeconds: 0 });
  });

  it("sends each tick's input to the server and predicts the tick from it as the server reads it", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    const input = { ...IDLE_INPUT, throttle: 1, steer: -0.5 };
    run.tick(input, 1 / TICKS_PER_SECOND);
    run.tick(IDLE_INPUT, 1 / TICKS_PER_SECOND);
    expect(client.inputs).toEqual([input, IDLE_INPUT]);
    expect(prediction().inputs).toEqual([quantiseInput(input, 1), quantiseInput(IDLE_INPUT, 2)]);
  });

  it("reconciles the prediction with every snapshot and reports its corrections per second", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    client.snapshot(snapshot(60));
    client.snapshot(snapshot(62));
    expect(prediction().snapshots.map((held) => held.tick)).toEqual([60, 62]);
    expect(run.correctionsPerSecond()).toBe(0.5);
  });

  it("draws the other karts where the interpolation puts them, the own kart where it is predicted", () => {
    const run = createServerRace(bridge, fakeClient(), COUNTDOWN, 1, "{}");
    const interpolated: KartTransform = {
      position: [10, 0.3, -20],
      rotation: [0, 0, 0, 1],
      speed: 12,
      surface: "sand",
      drift: { direction: "left", chargeSeconds: 1, miniTurbo: "blue" },
    };
    prediction().interpolated = Array<KartTransform>(KARTS).fill(interpolated);
    const [first, own, third] = run.kartTransforms();
    expect(first).toEqual(interpolated);
    expect(own).toEqual(PREDICTED);
    expect(third).toEqual(interpolated);
  });

  it("reads the race clock from the prediction and the lap clocks from the events", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    prediction().clock = { kind: "racing", elapsedSeconds: 50, graceLeftSeconds: null };
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
    prediction().clock = { kind: "racing", elapsedSeconds: 130, graceLeftSeconds: null };
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
