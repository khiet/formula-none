// How the client prints the sim's numbers, shared by the Instrument panel
// and the Results Scene so a time reads the same in both.

/** What stands in for a lap time there is none of yet. */
export const NO_LAP_TIME = "--:--.---";

/** 1st, 2nd, 3rd, 4th and so on, for the eight positions a race can have. */
export function ordinal(position: number): string {
  const suffix = { 1: "st", 2: "nd", 3: "rd" }[position] ?? "th";
  return `${position}${suffix}`;
}

/** Minutes, seconds and milliseconds: 1:15.457. */
export function formatLapTime(seconds: number): string {
  const wholeMilliseconds = Math.round(seconds * 1000);
  const minutes = Math.floor(wholeMilliseconds / 60_000);
  const rest = wholeMilliseconds - minutes * 60_000;
  const wholeSeconds = Math.floor(rest / 1000);
  const milliseconds = rest - wholeSeconds * 1000;
  return `${minutes}:${String(wholeSeconds).padStart(2, "0")}.${String(milliseconds).padStart(3, "0")}`;
}
