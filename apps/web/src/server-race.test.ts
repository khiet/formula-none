import { beforeEach, describe, expect, it } from "vitest";
import type { Bridge, KartInput, KartTransform, Phase, Prediction, Race } from "./bridge";
import { TOKEN } from "./fake-connection";
import type { Event, FinalStanding, Input, KartSnapshot, Snapshot } from "./generated/protocol";
import { IDLE_INPUT } from "./input";
import { ITEM_FIRE_FLASH_S } from "./race-scene";
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

const ITEM_BOX_RESPAWN_S = 3;
const PREDICTED_FIRE_TIMEOUT_S = 1;
const ITEM_BOXES = 8;

const GRID_TRANSFORM: KartTransform = {
  position: [1, 0.3, 2],
  rotation: [0, 0, 0, 1],
  speed: 0,
  surface: "asphalt",
  drift: null,
  heldItem: null,
};

const PREDICTED: KartTransform = {
  position: [3, 0.3, -40],
  rotation: [0, 0, 0, 1],
  speed: 20,
  surface: "asphalt",
  drift: null,
  heldItem: "hazard",
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
  itemBoxRespawnSeconds: ITEM_BOX_RESPAWN_S,
  predictedFireTimeoutSeconds: PREDICTED_FIRE_TIMEOUT_S,
  createRace: () =>
    ({
      kartTransforms: () => Array<KartTransform>(KARTS).fill(GRID_TRANSFORM),
      itemBoxes: () =>
        Array.from({ length: ITEM_BOXES }, (_, box) => ({ position: [box, 0, -70], yaw: 0 })),
      itemBoxesPresent: () => Array<boolean>(ITEM_BOXES).fill(true),
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
    token: TOKEN,
    room: { code: "BXQF", state: "racing", players: [] },
    race: undefined,
    replaced: false,
    closed: new Promise(() => {}),
    resume: () => Promise.resolve(),
    onHeard: () => () => {},
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
      heldItem: null,
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

  const DT = 1 / TICKS_PER_SECOND;
  const picked = (id: number, kart: number, itemBox = 2): Event => ({
    id,
    kind: { itemPicked: { kart, item: "boost", itemBox } },
  });
  const fired = (id: number, kart: number): Event => ({
    id,
    kind: { itemFired: { kart, item: "boost" } },
  });

  it("shows the item the server rolled for the local kart, not the prediction's, until the server fires it", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    expect(run.itemSlot()).toEqual({ item: null, firing: false });
    client.event(picked(1, 1));
    expect(run.itemSlot()).toEqual({ item: null, firing: false });
    client.event(picked(2, 0));
    expect(run.itemSlot()).toEqual({ item: "boost", firing: false });
    // A bot's fire is not the local kart's; the local kart's empties the
    // slot and shows the fire for the flash.
    client.event(fired(3, 1));
    expect(run.itemSlot()).toEqual({ item: "boost", firing: false });
    client.event(fired(4, 0));
    expect(run.itemSlot()).toEqual({ item: null, firing: true });
    for (let tick = 0; tick < ITEM_FIRE_FLASH_S * TICKS_PER_SECOND; tick++) {
      run.tick(IDLE_INPUT, DT);
    }
    expect(run.itemSlot()).toEqual({ item: null, firing: false });
  });

  it("predicts the fire from the press and reconciles it with the server's event", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    client.event(picked(1, 0));
    run.tick({ ...IDLE_INPUT, useItem: true }, DT);
    expect(run.itemSlot()).toEqual({ item: "boost", firing: true });
    run.tick(IDLE_INPUT, DT);
    expect(run.itemSlot()).toEqual({ item: "boost", firing: true });
    // The server's word: the item is gone, and the fire plays out.
    client.event(fired(2, 0));
    expect(run.itemSlot()).toEqual({ item: null, firing: true });
    for (let tick = 0; tick < ITEM_FIRE_FLASH_S * TICKS_PER_SECOND; tick++) {
      run.tick(IDLE_INPUT, DT);
    }
    expect(run.itemSlot()).toEqual({ item: null, firing: false });
    // A press with nothing held predicts nothing.
    run.tick({ ...IDLE_INPUT, useItem: true }, DT);
    expect(run.itemSlot()).toEqual({ item: null, firing: false });
  });

  it("gives up a predicted fire the server never confirms and shows the item still held", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    client.event(picked(1, 0));
    run.tick({ ...IDLE_INPUT, useItem: true }, DT);
    for (let tick = 1; tick < PREDICTED_FIRE_TIMEOUT_S * TICKS_PER_SECOND; tick++) {
      run.tick(IDLE_INPUT, DT);
      expect(run.itemSlot()).toEqual({ item: "boost", firing: true });
    }
    run.tick(IDLE_INPUT, DT);
    expect(run.itemSlot()).toEqual({ item: "boost", firing: false });
  });

  it("fills the slot from the snapshots until the first item event names the local kart, as after a rejoin", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    client.snapshot(snapshot(60, [{ heldItem: "homing" }]));
    expect(run.itemSlot()).toEqual({ item: "homing", firing: false });
    client.snapshot(snapshot(62, [{ heldItem: null }]));
    expect(run.itemSlot()).toEqual({ item: null, firing: false });
    // From the first event on, a snapshot sent before the fire no longer
    // puts the item back.
    client.event(picked(1, 0));
    client.event(fired(2, 0));
    client.snapshot(snapshot(64, [{ heldItem: "boost" }]));
    expect(run.itemSlot()).toEqual({ item: null, firing: true });
  });

  it("hides a box the server reports picked up for the tuned respawn", () => {
    const client = fakeClient();
    const run = createServerRace(bridge, client, COUNTDOWN, 0, "{}");
    expect(run.itemBoxes()).toHaveLength(ITEM_BOXES);
    expect(run.itemBoxesPresent()).toEqual(Array<boolean>(ITEM_BOXES).fill(true));
    client.event(picked(1, 3, 5));
    const present = run.itemBoxesPresent();
    expect(present[5]).toBe(false);
    expect(present.filter(Boolean)).toHaveLength(ITEM_BOXES - 1);
    for (let tick = 1; tick < ITEM_BOX_RESPAWN_S * TICKS_PER_SECOND; tick++) {
      run.tick(IDLE_INPUT, DT);
      expect(run.itemBoxesPresent()[5]).toBe(false);
    }
    run.tick(IDLE_INPUT, DT);
    expect(run.itemBoxesPresent()[5]).toBe(true);
  });
});
