import type { Standing } from "./bridge";
import { formatLapTime, NO_LAP_TIME, ordinal } from "./format";

/** What the Results Scene needs from a finished race. */
export interface RaceResults {
  /** The final standings, one per kart in grid order. */
  readonly standings: readonly Standing[];
  readonly localKart: number;
  isBot(slot: number): boolean;
  /** The nickname of the human driving `slot`, when the race had a room to say. */
  nickname?(slot: number): string | undefined;
}

/** One line of the results: a kart's place, who drove it and its best lap, as printed. */
export interface ResultRow {
  readonly position: string;
  readonly name: string;
  /** The best lap, or blank dashes for a kart that completed none. */
  readonly bestLap: string;
  /** Whether the kart completed the race's laps before the race ended. */
  readonly finished: boolean;
}

export interface ResultsScene {
  dispose(): void;
}

export interface ResultsHandlers {
  /** Rematch: another race on the same track. */
  rematch(): void;
  /** Leave: back to the Title Scene. */
  leave(): void;
}

const NOT_FINISHED = "(not finished)";

/**
 * The results from the final standings, in finishing order: the local kart
 * is "You", another human is their nickname when the room gave one, and
 * anyone else is named by their grid slot counted from 1.
 */
export function resultsRows({ standings, localKart, isBot, nickname }: RaceResults): ResultRow[] {
  const name = (slot: number): string => {
    if (slot === localKart) return "You";
    if (isBot(slot)) return `Bot ${slot + 1}`;
    return nickname?.(slot) ?? `Kart ${slot + 1}`;
  };
  return standings
    .map((standing, slot) => ({ standing, slot }))
    .sort((a, b) => a.standing.position - b.standing.position)
    .map(({ standing, slot }) => ({
      position: ordinal(standing.position),
      name: name(slot),
      bestLap:
        standing.bestLapSeconds === null ? NO_LAP_TIME : formatLapTime(standing.bestLapSeconds),
      finished: standing.finishTimeSeconds !== null,
    }));
}

/**
 * The Results Scene: the finishing order with each kart's best lap, and the
 * two ways on. Each row is addressable by `data-result` and each button by
 * its name, so tests and styles find them the same way.
 */
export function createResultsScene(
  container: HTMLElement,
  rows: readonly ResultRow[],
  handlers: ResultsHandlers,
): ResultsScene {
  const scene = document.createElement("div");
  scene.id = "results-scene";
  const heading = document.createElement("h1");
  heading.textContent = "Results";
  const list = document.createElement("ol");
  for (const row of rows) {
    const item = document.createElement("li");
    item.dataset.result = row.position;
    const cells = [row.position, row.name, row.bestLap];
    if (!row.finished) {
      cells.push(NOT_FINISHED);
    }
    item.textContent = cells.join(" ");
    list.append(item);
  }
  const button = (name: string, label: string, press: () => void): HTMLButtonElement => {
    const element = document.createElement("button");
    element.type = "button";
    element.name = name;
    element.textContent = label;
    element.addEventListener("click", press);
    return element;
  };
  scene.append(
    heading,
    list,
    button("rematch", "Rematch", handlers.rematch),
    button("leave", "Leave", handlers.leave),
  );
  container.append(scene);
  return {
    dispose() {
      scene.remove();
    },
  };
}
