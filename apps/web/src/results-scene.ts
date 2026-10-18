import type { Standing } from "./bridge";
import { formatLapTime, ordinal } from "./instrument-panel";

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

const NO_BEST_LAP = "--:--.---";
const NOT_FINISHED = "(not finished)";

/**
 * The results from the final standings, in finishing order: the local kart
 * is "You" and any other by its grid slot counted from 1, until rooms bring
 * nicknames.
 */
export function resultsRows(
  standings: readonly Standing[],
  localKart: number,
  isBot: (slot: number) => boolean,
): ResultRow[] {
  return standings
    .map((standing, slot) => ({ standing, slot }))
    .sort((a, b) => a.standing.position - b.standing.position)
    .map(({ standing, slot }) => ({
      position: ordinal(standing.position),
      name: slot === localKart ? "You" : `${isBot(slot) ? "Bot" : "Kart"} ${slot + 1}`,
      bestLap:
        standing.bestLapSeconds === null ? NO_BEST_LAP : formatLapTime(standing.bestLapSeconds),
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
