// A race run from the server's snapshots (ADR 0009): the player's own kart
// is predicted here from the inputs it sends the server each tick and
// reconciled with each snapshot, every other kart is interpolated between
// the snapshots the tuned delay behind the newest, and the readouts the
// Instrument panel needs that a snapshot does not carry are kept from the
// events. The race clock is the prediction's, so the countdown shown is the
// one the player's inputs meet on the server. Every item outcome is the
// server's event: the slot shows the item the server rolled and the boxes
// it reports taken, and a press only predicts the fire until the server's
// event confirms it. The hazards are the newest snapshot's; one a kart
// still trails is drawn behind that kart where the kart is drawn, so it
// follows the predicted own kart and the interpolated others. The
// projectiles are drawn between the snapshots as the other karts are, so
// one reaches its target as the target is drawn spinning.

import type { Bridge, Hazard, KartReadout, Phase, Projectile, Standing } from "./bridge";
import type { FinalStanding, Item, Snapshot } from "./generated/protocol";
import type { ItemSlot } from "./instrument-panel";
import { ITEM_FIRE_FLASH_S, type RaceRun } from "./race-scene";
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
  const ticks = (seconds: number) => Math.round(seconds * bridge.ticksPerSecond);
  /** The item the server says the local kart holds. */
  let heldItem: Item | null = null;
  /**
   * Whether an item event has named the local kart yet. Until one has, as
   * after a rejoin mid-race, the snapshots say what the slot holds; from
   * then on only the events do, so a snapshot sent before a fire cannot
   * put the item back.
   */
  let slotHeardOf = false;
  /** Ticks left of a fire predicted from a press the server has not confirmed; 0 for none. */
  let predictedFireTicksLeft = 0;
  /** Ticks left of the flash after the server's fire. */
  let flashTicksLeft = 0;
  const boxes = track.itemBoxes();
  /** Ticks until each box the server reported picked up is back; 0 while it is there. */
  const boxTicksLeft: number[] = Array<number>(boxes.length).fill(0);

  const stopSnapshots = client.onSnapshot((snapshot) => {
    latest = snapshot;
    prediction.reconcile(snapshot);
    if (!slotHeardOf) {
      heldItem = snapshot.karts[localKart]?.heldItem ?? null;
    }
  });
  const stopEvents = client.onEvent(({ kind }) => {
    if (typeof kind === "string") return;
    if ("lapCompleted" in kind) {
      lapTimes[kind.lapCompleted.kart]?.push(kind.lapCompleted.lapTimeS);
    } else if ("finished" in kind) {
      finishTimes[kind.finished.kart] = kind.finished.finishTimeS;
    } else if ("itemPicked" in kind) {
      const { kart, item, itemBox } = kind.itemPicked;
      if (itemBox < boxTicksLeft.length) {
        boxTicksLeft[itemBox] = ticks(bridge.itemBoxRespawnSeconds);
      }
      if (kart === localKart) {
        heldItem = item;
        slotHeardOf = true;
      }
    } else if ("itemFired" in kind && kind.itemFired.kart === localKart) {
      // The server's word on the fire, predicted or not: the item is gone
      // and the fire plays out.
      heldItem = null;
      slotHeardOf = true;
      predictedFireTicksLeft = 0;
      flashTicksLeft = ticks(ITEM_FIRE_FLASH_S);
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
      if (input.useItem && heldItem !== null && predictedFireTicksLeft === 0) {
        predictedFireTicksLeft = ticks(bridge.predictedFireTimeoutSeconds);
      } else {
        predictedFireTicksLeft = Math.max(predictedFireTicksLeft - 1, 0);
      }
      flashTicksLeft = Math.max(flashTicksLeft - 1, 0);
      for (let box = 0; box < boxTicksLeft.length; box++) {
        boxTicksLeft[box] = Math.max((boxTicksLeft[box] ?? 0) - 1, 0);
      }
    },
    kartReadouts() {
      const interpolated = prediction.interpolatedKartReadouts();
      const karts = interpolated.length === 0 ? track.kartReadouts() : interpolated;
      karts[localKart] = prediction.kartReadout();
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
          lapTimeS: finishTime === null ? Math.max(raceClock - raced, 0) : (laps.at(-1) ?? 0),
          bestLapS: final?.bestLapS ?? (laps.length === 0 ? null : Math.min(...laps)),
          wrongWay: snapshot?.wrongWay ?? false,
          finishTimeS: finishTime,
        };
      });
    },
    phase(): Phase {
      if (results !== undefined) return { kind: "finished" };
      // The prediction's clock never counts the grace, which is the
      // server's; the panel shows nothing of it.
      return prediction.phase();
    },
    itemSlot(): ItemSlot {
      return { item: heldItem, firing: predictedFireTicksLeft > 0 || flashTicksLeft > 0 };
    },
    itemBoxes: () => boxes,
    itemBoxesPresent: () => boxTicksLeft.map((left) => left <= 0),
    hazards(): Hazard[] {
      if (latest === undefined) return [];
      const karts = this.kartReadouts();
      return latest.hazards.map((hazard) => {
        const holder = hazard.held ? karts[hazard.owner] : undefined;
        return holder === undefined
          ? hazard
          : {
              ...hazard,
              position: behind(holder, bridge.hazardDropDistanceMetres, bridge.kartSize.height),
            };
      });
    },
    projectiles: (): Projectile[] => prediction.interpolatedProjectiles(),
    correctionsPerSecond: () => prediction.correctionsPerSecond(),
    dispose() {
      stopSnapshots();
      stopEvents();
      stopResults();
    },
  };
}

/** The point `distance` metres behind `kart`, on the surface under its centre. */
function behind(
  kart: KartReadout,
  distance: number,
  kartHeight: number,
): readonly [number, number, number] {
  // The kart faces -Z, so its rear is +Z turned by its yaw.
  const [qx, qy, qz, qw] = kart.rotation;
  const rearX = 2 * (qx * qz + qw * qy);
  const rearZ = 1 - 2 * (qx * qx + qy * qy);
  const [x, y, z] = kart.position;
  return [x + rearX * distance, y - kartHeight / 2, z + rearZ * distance];
}
