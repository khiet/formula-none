// A race run from the server's snapshots (ADR 0009): the player's own kart
// is predicted here from the inputs it sends the server each tick and
// reconciled with each snapshot, every other kart is interpolated between
// the snapshots the tuned delay behind the newest, and the readouts the Instrument panel needs that a
// snapshot does not carry are kept from the events. The race clock is the
// prediction's, so the countdown shown is the one the player's inputs meet
// on the server.

import type { Bridge, Phase, Standing } from "./bridge";
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
  // Never stepped: it stands the karts on the grid until the first snapshot.
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

  return {
    humanCount,
    localKart,
    tick(input) {
      const sequence = client.sendInput(input);
      prediction.tick(quantiseInput(input, sequence));
    },
    kartTransforms() {
      const interpolated = prediction.interpolatedKartTransforms();
      const karts = interpolated.length === 0 ? track.kartTransforms() : interpolated;
      karts[localKart] = prediction.kartTransform();
      return karts;
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
