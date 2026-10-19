// A race run from the server's snapshots (ADR 0009): the player's own kart
// is predicted here from the inputs it sends the server each tick and
// reconciled with each snapshot, every other kart is drawn where the last
// snapshot put it, and the readouts the Instrument panel needs that a
// snapshot does not carry are kept from the events. The race clock is the
// prediction's, so the countdown shown is the one the player's inputs meet
// on the server.

import type { Bridge, KartTransform, Phase, Standing } from "./bridge";
import type { FinalStanding, Snapshot } from "./generated/protocol";
import type { RaceRun } from "./race-scene";
import { type Countdown, quantiseInput, type RoomClient } from "./room-client";

export interface ServerRace extends RaceRun {
  /**
   * Reconciliation corrections of the own kart per second raced, which
   * the client reports at race end (ADR 0018).
   */
  correctionsPerSecond(): number;
  /** Stop following the room client; the run keeps its last readouts. */
  dispose(): void;
}

/**
 * The run of the race `countdown` announced, as the player in `localSlot`
 * sees it through `client`. The sidecar is the countdown's track, which
 * the sim reads for the surface under each kart and for where the grid
 * stands before the first snapshot.
 */
export function createServerRace(
  bridge: Bridge,
  client: RoomClient,
  countdown: Countdown,
  localSlot: number,
  sidecar: string,
): ServerRace {
  const humanCount = countdown.grid.length;
  const localKart = countdown.grid.indexOf(localSlot);
  if (localKart < 0) {
    throw new RangeError(`slot ${localSlot} is not on the grid ${countdown.grid.join(",")}`);
  }
  const kartCount = bridge.kartsPerRace;
  // Never stepped: it answers for the track and stands the karts on the grid.
  const track = bridge.createRace(countdown.seed, humanCount, kartCount - humanCount, sidecar);
  const prediction = bridge.createPrediction(countdown.seed, localKart, sidecar);

  let latest: Snapshot | undefined;
  /** Each kart's completed lap times, in order. */
  const lapTimes: number[][] = Array.from({ length: kartCount }, () => []);
  const finishTimes: (number | null)[] = Array<number | null>(kartCount).fill(null);
  let results: readonly FinalStanding[] | undefined;

  const stopSnapshots = client.onSnapshot((snapshot) => {
    latest = snapshot;
    prediction.reconcile(snapshot);
  });
  const stopEvents = client.onEvent(({ kind }) => {
    if (typeof kind === "string") return;
    if ("lapCompleted" in kind) {
      lapTimes[kind.lapCompleted.kart]?.push(kind.lapCompleted.lapTimeS);
    } else if ("finished" in kind) {
      finishTimes[kind.finished.kart] = kind.finished.finishTimeS;
    }
  });
  const stopResults = client.onResults((standings) => {
    results = standings;
  });

  /** Every kart where `snapshot` puts it, with the sim's surface under it. */
  function fromSnapshot(snapshot: Snapshot): KartTransform[] {
    return snapshot.karts.map((kart) => ({
      position: kart.position,
      rotation: kart.rotation,
      speed: speedAlongFacing(kart.velocity, kart.rotation),
      surface: track.surfaceAt(kart.position),
      drift:
        kart.drift === null
          ? null
          : {
              direction: kart.drift.direction,
              chargeSeconds: kart.drift.chargeS,
              miniTurbo: bridge.miniTurboTier(kart.drift.chargeS),
            },
    }));
  }

  return {
    humanCount,
    localKart,
    tick(input) {
      const sequence = client.sendInput(input);
      prediction.tick(quantiseInput(input, sequence));
    },
    kartTransforms() {
      const others = latest === undefined ? track.kartTransforms() : fromSnapshot(latest);
      others[localKart] = prediction.kartTransform();
      return others;
    },
    standings() {
      const clock = prediction.phase();
      const raceClock = clock.kind === "racing" ? clock.elapsedSeconds : 0;
      return Array.from({ length: kartCount }, (_, kart): Standing => {
        const snapshot = latest?.karts[kart];
        const final = results?.find((standing) => standing.kart === kart);
        const laps = lapTimes[kart] ?? [];
        const finishTime = final?.finishTimeS ?? finishTimes[kart] ?? null;
        const raced = laps.reduce((sum, lap) => sum + lap, 0);
        return {
          position: final?.position ?? snapshot?.racePosition ?? kart + 1,
          lap: snapshot?.lap ?? 1,
          checkpoint: snapshot?.checkpoint ?? 0,
          splineProgress: snapshot?.splineProgress ?? 0,
          // The lap clock runs from Go, less the laps already completed; a
          // finisher's stops on the last lap's time.
          lapTimeSeconds: finishTime === null ? Math.max(raceClock - raced, 0) : (laps.at(-1) ?? 0),
          bestLapSeconds: final?.bestLapS ?? (laps.length === 0 ? null : Math.min(...laps)),
          wrongWay: snapshot?.wrongWay ?? false,
          finishTimeSeconds: finishTime,
        };
      });
    },
    phase(): Phase {
      if (results !== undefined) return { kind: "finished" };
      // The prediction's clock never counts the grace, which is the
      // server's; the panel shows nothing of it.
      return prediction.phase();
    },
    correctionsPerSecond: () => prediction.correctionsPerSecond(),
    dispose() {
      stopSnapshots();
      stopEvents();
      stopResults();
    },
  };
}

/**
 * The velocity's component along the kart's facing: the speed the panel
 * prints, negative when backing up. A kart with identity rotation faces
 * -Z; the quaternion is [x, y, z, w].
 */
export function speedAlongFacing(
  velocity: readonly [number, number, number],
  rotation: readonly [number, number, number, number],
): number {
  const [x, y, z, w] = rotation;
  // -Z rotated by the quaternion: the negated third column of its matrix.
  const forward = [-2 * (x * z + w * y), -2 * (y * z - w * x), -(1 - 2 * (x * x + y * y))];
  return (
    velocity[0] * (forward[0] ?? 0) +
    velocity[1] * (forward[1] ?? 0) +
    velocity[2] * (forward[2] ?? 0)
  );
}
