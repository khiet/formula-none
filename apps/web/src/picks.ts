// The creator's picks as the Title Scene offers them and the Room Scene
// labels them: a heading per pick and a caption per value. What the values
// are and which a fresh creator starts from is the sim's (`PickChoices`,
// `Picks`); this only names them.
import type { BotLevel, PickChoices, Picks, Tyres, Weather } from "./generated/sim";
import type { PickOffer } from "./track";

/** A value as the Title Scene shows it: its tile's caption and the line describing the pick. */
export interface PickCaption {
  readonly name: string;
  /** What the value means for the race, in the course blurb's voice, after "Name: "; none draws no line. */
  readonly blurb?: string;
}

/** One pick as the Title Scene offers it. */
export interface PickRow<Value> {
  /**
   * The radio group's name, and the key the pick is remembered under and a
   * tester asks for it by in the query string (ADR 0078).
   */
  readonly key: string;
  readonly heading: string;
  caption(value: Value): PickCaption;
}

const LEVELS: Readonly<Record<BotLevel, PickCaption>> = {
  basic: { name: "Basic", blurb: "lift for corners, hold an item, wait for you" },
  pro: { name: "Pro", blurb: "carry speed, fire sooner, wait less" },
  ace: { name: "Ace", blurb: "corner flat out, fire on sight, never wait for you" },
};

const TYRES: Readonly<Record<Tyres, PickCaption>> = {
  off: { name: "Off", blurb: "they last the race" },
  on: { name: "On", blurb: "driving wears them" },
};

const WEATHER: Readonly<Record<Weather, PickCaption>> = {
  dry: { name: "Dry", blurb: "it stays dry" },
  rain: { name: "Rain", blurb: "grip falls mid-race" },
};

/** Every pick in the Title Scene's order, under the flags (ADR 0078, ADR 0083, ADR 0084, ADR 0103). */
export const PICK_ROWS: { readonly [Pick in keyof Picks]: PickRow<Picks[Pick]> } = {
  level: { key: "bots", heading: "Bots", caption: (level) => LEVELS[level] },
  tyres: { key: "tyres", heading: "Tyres", caption: (tyres) => TYRES[tyres] },
  laps: { key: "laps", heading: "Laps", caption: (laps) => ({ name: String(laps) }) },
  weather: { key: "weather", heading: "Weather", caption: (weather) => WEATHER[weather] },
};

/** What a creator is offered of each pick: its row's key and the sim's `choices`. */
export function pickOffer(choices: PickChoices): PickOffer {
  return {
    level: { key: PICK_ROWS.level.key, values: choices.level },
    tyres: { key: PICK_ROWS.tyres.key, values: choices.tyres },
    laps: { key: PICK_ROWS.laps.key, values: choices.laps },
    weather: { key: PICK_ROWS.weather.key, values: choices.weather },
  };
}
