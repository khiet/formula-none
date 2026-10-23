// The three audio toggles (ADR 0008), kept in localStorage next to the
// nickname so a player who mutes the music stays muted on the next visit.

const STORAGE_KEY = "audio";

export interface AudioSettings {
  /** The hop, mini-turbo, item, hit and lap sounds. */
  readonly sfx: boolean;
  /** The engine note under the own kart. */
  readonly engine: boolean;
  /** The music loop through a race. */
  readonly music: boolean;
}

/** Everything on: the game sounds until the player says otherwise. */
export const DEFAULT_AUDIO_SETTINGS: AudioSettings = { sfx: true, engine: true, music: true };

/** The saved toggles, each at its default when unsaved, unreadable or not a boolean. */
export function loadAudioSettings(storage: Storage): AudioSettings {
  let saved: unknown;
  try {
    saved = JSON.parse(storage.getItem(STORAGE_KEY) ?? "null");
  } catch {
    return DEFAULT_AUDIO_SETTINGS;
  }
  if (typeof saved !== "object" || saved === null || Array.isArray(saved)) {
    return DEFAULT_AUDIO_SETTINGS;
  }
  const toggles = saved as Record<string, unknown>;
  const toggle = (name: keyof AudioSettings): boolean =>
    typeof toggles[name] === "boolean" ? toggles[name] : DEFAULT_AUDIO_SETTINGS[name];
  return { sfx: toggle("sfx"), engine: toggle("engine"), music: toggle("music") };
}

/** Remember the toggles for the next visit; storage that refuses is let be. */
export function saveAudioSettings(storage: Storage, settings: AudioSettings): void {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // A private window or a full store; the next visit loads what was saved before.
  }
}
