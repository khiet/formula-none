/** How long the BOX call shows on the Instrument panel. Presentation, so not the sim's to tune. */
export const BOX_CALL_S = 2;

export interface BoxCallRules {
  /** Whether the course has a pit lane to come in to; without one there is nothing to call. */
  readonly pitLane: boolean;
  readonly lapsPerRace: number;
  /** The tyre life, as a percentage, the call comes at. */
  readonly cliff: number;
}

export interface BoxCall {
  /** One frame of the own kart at `tyreLife` on `lap`; returns whether the call is showing. */
  update(tyreLife: number, lap: number, frameSeconds: number): boolean;
}

/**
 * The radio call to come in for tyres (ADR 0080): once a race, as life first
 * falls to the cliff, and never for a kart that reaches it on the final lap,
 * where a stop could not pay. Latched, so a reconciliation that moves life
 * back over the cliff does not call it again, nor will a later stop.
 */
export function createBoxCall(rules: BoxCallRules): BoxCall {
  let reached = false;
  let leftSeconds = 0;
  return {
    update(tyreLife, lap, frameSeconds) {
      leftSeconds = Math.max(leftSeconds - frameSeconds, 0);
      if (!reached && tyreLife <= rules.cliff) {
        reached = true;
        if (rules.pitLane && lap < rules.lapsPerRace) leftSeconds = BOX_CALL_S;
      }
      return leftSeconds > 0;
    },
  };
}
