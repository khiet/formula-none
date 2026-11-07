import type { PitLaneFork, Tuning } from "./bridge";

/**
 * How far short of the pit lane's fork the BOX call opens: about 4 s at top
 * speed to hear it and move across. Presentation, so not the sim's to tune.
 */
export const BOX_CALL_FROM_M = 120;

/**
 * How far short of the fork the call closes: inside it there is no longer
 * room to get across. Presentation, so not the sim's to tune.
 */
export const BOX_CALL_UNTIL_M = 15;

export interface BoxCallRules {
  /** Where the course's pit lane forks off; without one there is nothing to call. */
  readonly fork: PitLaneFork | undefined;
  readonly lapsPerRace: number;
}

/** What the call reads of the own kart each frame. */
export interface BoxCallKart {
  readonly tyreLife: number;
  readonly lap: number;
  /** Metres along the centreline from the start line, in `0..spline length`. */
  readonly splineProgress: number;
  readonly inPitLane: boolean;
}

export interface BoxCall {
  /** One frame of the own kart; returns whether the call is showing. */
  update(kart: BoxCallKart): boolean;
}

/**
 * The radio call to come in for tyres: shows while the own kart, at or below
 * the tuned cliff, out of the pit lane and short of the final lap, is between
 * `BOX_CALL_FROM_M` and `BOX_CALL_UNTIL_M` short of the fork. It opens at most
 * once for each pass of the fork, so a reconciliation that moves progress or
 * life back over an edge neither reopens it nor sounds the chime again.
 */
export function createBoxCall(
  rules: BoxCallRules,
  tuning: Pick<Tuning, "tyreLifeCliffPercent">,
): BoxCall {
  const { fork } = rules;
  // The lap on which the kart reaches the fork the last call opened for.
  let calledFor: number | undefined;
  let showing = false;
  return {
    update(kart) {
      if (fork === undefined) return false;
      const metres = metresToTheFork(fork, kart.splineProgress);
      // A fork just past the line from here is the next lap's.
      const forkLap = kart.splineProgress <= fork.splineProgress ? kart.lap : kart.lap + 1;
      const holds =
        metres <= BOX_CALL_FROM_M &&
        metres > BOX_CALL_UNTIL_M &&
        !kart.inPitLane &&
        kart.lap < rules.lapsPerRace &&
        kart.tyreLife <= tuning.tyreLifeCliffPercent;
      if (showing) {
        showing = holds && forkLap === calledFor;
      } else if (holds && forkLap !== calledFor) {
        showing = true;
        calledFor = forkLap;
      }
      return showing;
    },
  };
}

/** Metres along the centreline from `splineProgress` on to the fork, wrapping at the line. */
function metresToTheFork(fork: PitLaneFork, splineProgress: number): number {
  const metres = (fork.splineProgress - splineProgress) % fork.splineLength;
  return metres < 0 ? metres + fork.splineLength : metres;
}
