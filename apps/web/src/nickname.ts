// The nickname a player types once on the Title Scene and keeps across
// visits, validated against the sim's tuned bounds as the bridge reports
// them (ADR 0014).

import type { Tuning } from "./bridge";

const STORAGE_KEY = "nickname";

/** Bounds on a nickname's length in characters, inclusive. */
export interface NicknameChars {
  readonly min: number;
  readonly max: number;
}

/** The sim's nickname bounds as the Title Scene checks them. */
export function nicknameChars(
  tuning: Pick<Tuning, "nicknameMinChars" | "nicknameMaxChars">,
): NicknameChars {
  return { min: tuning.nicknameMinChars, max: tuning.nicknameMaxChars };
}

export type NicknameCheck =
  | { readonly ok: true; readonly nickname: string }
  | { readonly ok: false; readonly message: string };

/** The nickname as it will be sent, trimmed, or why it cannot be. */
export function validateNickname(typed: string, chars: NicknameChars): NicknameCheck {
  const nickname = typed.trim();
  const length = [...nickname].length;
  if (length < chars.min || length > chars.max) {
    return { ok: false, message: `Nickname needs ${chars.min} to ${chars.max} characters` };
  }
  return { ok: true, nickname };
}

/** The saved nickname, or empty when none is saved or storage is unavailable. */
export function loadNickname(storage: Storage): string {
  try {
    return storage.getItem(STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

/** Remember the nickname for the next visit; storage that refuses is let be. */
export function saveNickname(storage: Storage, nickname: string): void {
  try {
    storage.setItem(STORAGE_KEY, nickname);
  } catch {
    // A private window or a full store: the player types it again next time.
  }
}
