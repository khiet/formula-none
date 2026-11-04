import type { Standing } from "./bridge";
import { button } from "./controls";
import { formatLapTime, formatStopTime, NO_LAP_TIME, ordinal } from "./format";
import { livery, liveryCss } from "./livery";
import { backdrop, kartSide } from "./poster";

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
  /** The kart's grid slot, which picks its livery and driver. */
  readonly slot: number;
  /** The place as a number, 1 for the winner. */
  readonly place: number;
  readonly name: string;
  /** The kart's livery as CSS. */
  readonly livery: string;
  /** The best lap; "Not finished" for a kart that did not finish, blank dashes for one that completed no lap. */
  readonly bestLap: string;
  /** Whether this kart finished with the lowest best lap among finishers; every kart that ties it holds it too. */
  readonly fastestLap: boolean;
  /** Whether the kart completed the race's laps before the race ended. */
  readonly finished: boolean;
  /** The kart's stops and the best of them, as printed; empty for a kart that made none (ADR 0082). */
  readonly stops: string;
  readonly you: boolean;
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

const NOT_FINISHED = "Not finished";
const PODIUM_PLACES = 3;
const CONFETTI_PIECES = 60;
const CONFETTI_COLOURS = ["#ffd23f", "#fff", "#111", "#3ddc84", "#4cc9f0"];

/**
 * Who drove `slot`, as the race names them: the local kart is "You", a bot
 * is "Bot" and its grid slot counted from 1, and another human is their
 * nickname when the room gave one, else "Kart" and their slot.
 */
export function kartName(
  { localKart, isBot, nickname }: Pick<RaceResults, "localKart" | "isBot" | "nickname">,
  slot: number,
): string {
  if (slot === localKart) return "You";
  if (isBot(slot)) return `Bot ${slot + 1}`;
  return nickname?.(slot) ?? `Kart ${slot + 1}`;
}

/**
 * The results from the final standings, in finishing order.
 */
export function resultsRows(results: RaceResults): ResultRow[] {
  // Only finishers count: a kart that did not finish shows no time to hold the badge against.
  const bestLaps = results.standings.flatMap((standing) =>
    standing.bestLapS === null || standing.finishTimeS === null ? [] : [standing.bestLapS],
  );
  const fastest = Math.min(...bestLaps);
  return results.standings
    .map((standing, slot) => ({ standing, slot }))
    .sort((a, b) => a.standing.position - b.standing.position)
    .map(({ standing, slot }) => {
      const finished = standing.finishTimeS !== null;
      return {
        position: ordinal(standing.position),
        slot,
        place: standing.position,
        name: kartName(results, slot),
        livery: liveryCss(slot),
        bestLap: !finished
          ? NOT_FINISHED
          : standing.bestLapS === null
            ? NO_LAP_TIME
            : formatLapTime(standing.bestLapS),
        fastestLap: finished && standing.bestLapS === fastest,
        finished,
        stops: stopsText(standing.stops, standing.bestStopS),
        you: slot === results.localKart,
      };
    });
}

/** The stops as the board prints them: none for no stop, the one stop's time, or the count and the best. */
export function stopsText(stops: number, bestStopS: number | null): string {
  if (stops === 0 || bestStopS === null) return "";
  if (stops === 1) return `1 stop, ${formatStopTime(bestStopS)}`;
  return `${stops} stops, best ${formatStopTime(bestStopS)}`;
}

/**
 * What the Scene says to you about your finish: a headline, a line under it
 * from 4th down, whether the place is stamped on and whether confetti falls.
 */
interface Verdict {
  readonly text: string;
  readonly subline: string;
  readonly stamp: boolean;
  readonly confetti: boolean;
}

function verdict(you: ResultRow | undefined): Verdict {
  const plain = { subline: "", stamp: false, confetti: false };
  if (you === undefined) return { ...plain, text: "Results" };
  if (!you.finished) return { ...plain, text: "Out of road!" };
  if (you.place === 1) return { ...plain, text: "You win!", confetti: true };
  if (you.place <= PODIUM_PLACES) {
    return { ...plain, text: `Podium! ${you.position}`, confetti: true };
  }
  const off = you.place - PODIUM_PLACES;
  return {
    text: you.position,
    subline: `${off} kart${off === 1 ? "" : "s"} off the podium. One more go?`,
    stamp: true,
    confetti: false,
  };
}

/** A kart's line: its place, name, best lap, any fastest-lap badge and its stops, each addressable by a data attribute. */
function resultItem(row: ResultRow, cells: { kart: boolean }): HTMLLIElement {
  const item = document.createElement("li");
  item.dataset.result = row.position;
  item.style.setProperty("--livery", row.livery);
  if (row.you) item.dataset.you = "";
  if (!row.finished) item.dataset.notFinished = "";
  const cell = (name: string, text: string): HTMLSpanElement => {
    const span = document.createElement("span");
    span.dataset[name] = "";
    span.textContent = text;
    return span;
  };
  item.append(
    cells.kart ? kartSide(livery(row.slot)) : cell("swatch", ""),
    cell("place", cells.kart ? String(row.place) : row.position),
    cell("name", row.name),
    cell("bestLap", row.bestLap),
  );
  if (row.fastestLap) item.append(cell("fastestLap", "Fastest lap"));
  if (row.stops !== "") item.append(cell("stops", row.stops));
  return item;
}

function confetti(): HTMLDivElement {
  const layer = document.createElement("div");
  layer.dataset.confetti = "";
  layer.setAttribute("aria-hidden", "true");
  for (let index = 0; index < CONFETTI_PIECES; index += 1) {
    const piece = document.createElement("i");
    piece.style.setProperty("--x", `${Math.random() * 100}%`);
    piece.style.setProperty("--c", CONFETTI_COLOURS[index % CONFETTI_COLOURS.length] ?? "#fff");
    piece.style.setProperty("--r", `${Math.random() * 360}deg`);
    piece.style.setProperty("--d", `${2.5 + Math.random() * 2.5}s`);
    piece.style.setProperty("--delay", `${-Math.random() * 4}s`);
    layer.append(piece);
  }
  return layer;
}

/**
 * The Results Scene as a Grand Prix poster: a headline for your finish, the
 * top three on a podium, the rest on a board beside it, and the two ways on.
 * Each row is addressable by `data-result` and each button by its name, so
 * tests and styles find them the same way. The reveal is styling only; the
 * buttons take a press from the first frame.
 */
export function createResultsScene(
  container: HTMLElement,
  rows: readonly ResultRow[],
  handlers: ResultsHandlers,
): ResultsScene {
  const scene = document.createElement("div");
  scene.id = "results-scene";
  const said = verdict(rows.find((row) => row.you));

  const head = document.createElement("header");
  const raceOver = document.createElement("p");
  raceOver.dataset.raceOver = "";
  raceOver.textContent = "Race over";
  const heading = document.createElement("h1");
  heading.textContent = said.text;
  if (said.stamp) heading.dataset.stamp = "";
  head.append(raceOver, heading);
  if (said.subline !== "") {
    const subline = document.createElement("p");
    subline.dataset.subline = "";
    subline.textContent = said.subline;
    head.append(subline);
  }

  const podium = document.createElement("ol");
  podium.dataset.podium = "";
  const board = document.createElement("ol");
  board.dataset.board = "";
  for (const row of rows) {
    if (row.place <= PODIUM_PLACES) {
      podium.append(resultItem(row, { kart: true }));
    } else {
      board.append(resultItem(row, { kart: false }));
    }
  }
  const actions = document.createElement("div");
  actions.dataset.actions = "";
  actions.append(
    button("rematch", "Rematch", handlers.rematch),
    button("leave", "Leave", handlers.leave),
  );
  const side = document.createElement("div");
  side.dataset.side = "";
  side.append(board, actions);

  scene.append(...backdrop(), head, podium, side);
  if (said.confetti) scene.append(confetti());
  container.append(scene);
  return {
    dispose() {
      scene.remove();
    },
  };
}
