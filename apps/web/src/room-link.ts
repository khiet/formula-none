// A room is shared as a link that carries its code: the page's
// own URL with `?room=CODE`, so opening it lands in the room. The code is
// read however it was typed; the server reads it in upper case.

const ROOM_PARAM = "room";

/** A code as the server reads it: upper case, no whitespace. */
export function normaliseCode(typed: string): string {
  return typed.replace(/\s+/g, "").toUpperCase();
}

/** The code a shared link carries, or null when the page was opened plain. */
export function roomFromSearch(search: string): string | null {
  const code = new URLSearchParams(search).get(ROOM_PARAM);
  if (code === null) return null;
  const normalised = normaliseCode(code);
  return normalised === "" ? null : normalised;
}

/** The query string that reopens the room, for the address bar. */
export function roomSearch(code: string): string {
  return `?${ROOM_PARAM}=${encodeURIComponent(code)}`;
}

/** The link to send a friend. */
export function roomLink(location: { origin: string; pathname: string }, code: string): string {
  return `${location.origin}${location.pathname}${roomSearch(code)}`;
}
