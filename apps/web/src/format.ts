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
  const wholeMilliseconds = Math.round(seconds * 1000);
  const minutes = Math.floor(wholeMilliseconds / 60_000);
  const rest = wholeMilliseconds - minutes * 60_000;
  const wholeSeconds = Math.floor(rest / 1000);
  const milliseconds = rest - wholeSeconds * 1000;
  return `${minutes}'${String(wholeSeconds).padStart(2, "0")}"${String(milliseconds).padStart(3, "0")}`;
}
