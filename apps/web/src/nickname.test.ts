import { describe, expect, it } from "vitest";
import { loadNickname, saveNickname, validateNickname } from "./nickname";

const CHARS = { min: 2, max: 12 };

/** The slice of Storage the nickname uses, in memory. */
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

describe("validateNickname", () => {
  it("accepts two to twelve characters, trimmed", () => {
    expect(validateNickname("ab", CHARS)).toEqual({ ok: true, nickname: "ab" });
    expect(validateNickname("  khiet ", CHARS)).toEqual({ ok: true, nickname: "khiet" });
    expect(validateNickname("twelve chars", CHARS)).toEqual({ ok: true, nickname: "twelve chars" });
  });

  it("refuses too short and too long, saying which", () => {
    expect(validateNickname("a", CHARS)).toEqual({
      ok: false,
      message: "Nickname needs 2 to 12 characters",
    });
    expect(validateNickname("   ", CHARS)).toEqual({
      ok: false,
      message: "Nickname needs 2 to 12 characters",
    });
    expect(validateNickname("thirteen chars", CHARS)).toEqual({
      ok: false,
      message: "Nickname needs 2 to 12 characters",
    });
  });

  it("counts characters, not bytes", () => {
    expect(validateNickname("éé", CHARS).ok).toBe(true);
    expect(validateNickname("🏁🏁", CHARS).ok).toBe(true);
  });
});

describe("nickname persistence", () => {
  it("is empty before one is saved and remembered after", () => {
    const store = storage();
    expect(loadNickname(store)).toBe("");
    saveNickname(store, "khiet");
    expect(loadNickname(store)).toBe("khiet");
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
    expect(loadNickname(broken)).toBe("");
    expect(() => saveNickname(broken, "khiet")).not.toThrow();
  });
});
