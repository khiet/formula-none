// The session token of the room the player is in, kept across a closed
// tab: reopening the room's link mid-race rejoins the same kart with it
// (ADR 0012). One room's at a time, in local storage, so a new tab of the
// same browser finds it.

import { normaliseCode } from "./room-link";

const STORAGE_KEY = "session";

export interface RoomSession {
  /** The room's code as the server gives it. */
  readonly code: string;
  readonly token: string;
}

/** The token dealt in the room with `code`, or undefined when none is kept or storage is unavailable. */
export function loadSession(storage: Storage, code: string): string | undefined {
  try {
    const kept: unknown = JSON.parse(storage.getItem(STORAGE_KEY) ?? "null");
    if (typeof kept !== "object" || kept === null) return undefined;
    const { code: keptCode, token } = kept as Partial<Record<keyof RoomSession, unknown>>;
    return keptCode === normaliseCode(code) && typeof token === "string" ? token : undefined;
  } catch {
    return undefined;
  }
}

/** Keep the session, in place of any other room's; storage that refuses is let be. */
export function saveSession(storage: Storage, session: RoomSession): void {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(session));
  } catch {
    // A private window or a full store: a reopened link joins afresh.
  }
}

/** Forget the session, as on leaving the room on purpose. */
export function forgetSession(storage: Storage): void {
  try {
    storage.removeItem(STORAGE_KEY);
  } catch {
    // Nothing kept, then.
  }
}
