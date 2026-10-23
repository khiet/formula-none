import { describe, expect, it } from "vitest";
import { DEFAULT_AUDIO_SETTINGS, loadAudioSettings, saveAudioSettings } from "./audio-settings";

/** The slice of Storage the settings use, in memory. */
function storage(initial: Record<string, string> = {}): Storage {
  const items = new Map(Object.entries(initial));
  return {
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
    clear: () => items.clear(),
    key: (index) => [...items.keys()][index] ?? null,
    get length() {
      return items.size;
    },
  };
}

describe("audio settings persistence", () => {
  it("has every toggle on before any is saved, and remembers what is saved", () => {
    const store = storage();
    expect(loadAudioSettings(store)).toEqual({ sfx: true, engine: true, music: true });
    expect(DEFAULT_AUDIO_SETTINGS).toEqual({ sfx: true, engine: true, music: true });
    saveAudioSettings(store, { sfx: true, engine: false, music: false });
    expect(loadAudioSettings(store)).toEqual({ sfx: true, engine: false, music: false });
  });

  it("keeps its own key next to the nickname", () => {
    const store = storage({ nickname: "khiet" });
    saveAudioSettings(store, { sfx: false, engine: true, music: true });
    expect(store.getItem("nickname")).toBe("khiet");
    expect(store.length).toBe(2);
  });

  it("falls back to the defaults for a saved value it cannot read", () => {
    expect(loadAudioSettings(storage({ audio: "not json" }))).toEqual(DEFAULT_AUDIO_SETTINGS);
    expect(loadAudioSettings(storage({ audio: "[1,2]" }))).toEqual(DEFAULT_AUDIO_SETTINGS);
    // A toggle missing or of the wrong type takes its default; the rest are kept.
    expect(loadAudioSettings(storage({ audio: '{"music":false,"sfx":"no"}' }))).toEqual({
      sfx: true,
      engine: true,
      music: false,
    });
  });

  it("survives storage that throws, as a private window's can", () => {
    const broken: Storage = {
      ...storage(),
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(loadAudioSettings(broken)).toEqual(DEFAULT_AUDIO_SETTINGS);
    expect(() => saveAudioSettings(broken, DEFAULT_AUDIO_SETTINGS)).not.toThrow();
  });
});
