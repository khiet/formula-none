import { describe, expect, it } from "vitest";
import { forgetSession, loadSession, saveSession } from "./room-session";

/** The slice of Storage the session uses, in memory. */
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

const TOKEN = "0123456789abcdef0123456789abcdef";

describe("the room session a reopened link rejoins with", () => {
  it("gives back the token for the room it was dealt in, however the code is typed", () => {
    const store = storage();
    saveSession(store, { code: "BXQF", token: TOKEN });
    expect(loadSession(store, "BXQF")).toBe(TOKEN);
    expect(loadSession(store, " bxqf ")).toBe(TOKEN);
    expect(loadSession(store, "ZZZZ")).toBeUndefined();
  });

  it("keeps one room's: joining another replaces it, and leaving forgets it", () => {
    const store = storage();
    saveSession(store, { code: "BXQF", token: TOKEN });
    saveSession(store, { code: "ZZZZ", token: "f".repeat(32) });
    expect(loadSession(store, "BXQF")).toBeUndefined();
    forgetSession(store);
    expect(loadSession(store, "ZZZZ")).toBeUndefined();
  });

  it("reads nothing from a store that holds something else or refuses", () => {
    expect(loadSession(storage({ session: "not json" }), "BXQF")).toBeUndefined();
    expect(loadSession(storage({ session: '{"code":"BXQF"}' }), "BXQF")).toBeUndefined();
    const refusing = {
      ...storage(),
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(loadSession(refusing, "BXQF")).toBeUndefined();
    expect(() => saveSession(refusing, { code: "BXQF", token: TOKEN })).not.toThrow();
  });
});
