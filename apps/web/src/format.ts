// How the client prints the sim's numbers, shared by the Instrument panel
// and the Results Scene so a time reads the same in both.

/** What stands in for a lap time there is none of yet, the same shape as a time. */
export const NO_LAP_TIME = `-'--"---`;

/** An ordinal in two parts, so the panel can set the numeral large and the suffix small. */
export interface OrdinalParts {
  readonly numeral: string;
  readonly suffix: string;
}

/** 1 and st, 2 and nd, 3 and rd, 4 and th and so on, for the eight positions a race can have. */
export function ordinalParts(position: number): OrdinalParts {
  const suffix = { 1: "st", 2: "nd", 3: "rd" }[position] ?? "th";
  return { numeral: String(position), suffix };
}

/** 1st, 2nd, 3rd, 4th and so on. */
export function ordinal(position: number): string {
  const { numeral, suffix } = ordinalParts(position);
  return `${numeral}${suffix}`;
}

/**
 * The 16-bit racer's notation at the sim's precision: minutes, seconds and
 * thousandths as 1'15"457 (ADR 0023).
 */
export function formatLapTime(seconds: number): string {
  const { minutes, wholeSeconds, milliseconds } = clockParts(seconds);
  return `${minutes}'${wholeSeconds}"${String(milliseconds).padStart(3, "0")}`;
}

/**
 * The lap clock while a lap runs: minutes, seconds and tenths as 1'15"4
 * (ADR 0039). The tenths are truncated from the thousandths formatLapTime
 * would print, so the running clock never reads ahead of the final time.
 */
export function formatRunningLapTime(seconds: number): string {
  const { minutes, wholeSeconds, milliseconds } = clockParts(seconds);
  return `${minutes}'${wholeSeconds}"${Math.floor(milliseconds / 100)}`;
}

/** `seconds` rounded to the millisecond, as whole minutes, two-digit seconds and milliseconds. */
function clockParts(seconds: number) {
  const wholeMilliseconds = Math.round(seconds * 1000);
  const minutes = Math.floor(wholeMilliseconds / 60_000);
  const rest = wholeMilliseconds - minutes * 60_000;
  const wholeSeconds = Math.floor(rest / 1000);
  const milliseconds = rest - wholeSeconds * 1000;
  return { minutes, wholeSeconds: String(wholeSeconds).padStart(2, "0"), milliseconds };
}

/** Metres per second as kilometres per hour, keeping the sign. */
export function kmPerHour(metresPerSecond: number): number {
  return metresPerSecond * 3.6;
}

/** A speed in two parts, so the panel can set the number large and the unit small. */
export interface SpeedParts {
  readonly value: string;
  readonly unit: string;
}

/** Speed as the panel prints it: whole km/h, positive either way the kart goes. */
export function speedParts(metresPerSecond: number): SpeedParts {
  return { value: String(Math.round(Math.abs(kmPerHour(metresPerSecond)))), unit: "km/h" };
}
