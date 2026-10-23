// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { type AudioSettings, DEFAULT_AUDIO_SETTINGS } from "./audio-settings";
import { createTitleScene, type TitleHandlers, type TitleOptions } from "./title-scene";

const OPTIONS: TitleOptions = {
  nickname: "",
  code: "",
  message: "",
  audio: DEFAULT_AUDIO_SETTINGS,
  nicknameChars: { min: 2, max: 12 },
  roomCodeLength: 4,
};

interface Calls {
  created: string[];
  joined: [string, string][];
  races: number;
  audio: AudioSettings[];
}

function scene(options: Partial<TitleOptions> = {}) {
  const container = document.createElement("div");
  document.body.replaceChildren(container);
  const calls: Calls = { created: [], joined: [], races: 0, audio: [] };
  const handlers: TitleHandlers = {
    create: (nickname) => calls.created.push(nickname),
    join: (nickname, code) => calls.joined.push([nickname, code]),
    race: () => {
      calls.races += 1;
    },
    audio: (settings) => calls.audio.push(settings),
  };
  const title = createTitleScene(container, { ...OPTIONS, ...options }, handlers);
  const input = (name: string) => {
    const element = container.querySelector<HTMLInputElement>(`input[name=${name}]`);
    if (!element) throw new Error(`no input ${name}`);
    return element;
  };
  const press = (name: string) => {
    container.querySelector<HTMLButtonElement>(`button[name=${name}]`)?.click();
  };
  const message = () => container.querySelector("[data-message]")?.textContent ?? "";
  return { container, title, calls, input, press, message };
}

describe("Title Scene", () => {
  it("names the game and offers the nickname, a room to create or join, and a solo race", () => {
    const { container, title, calls, press } = scene();
    expect(container.querySelector("h1")?.textContent).toBe("Formula None");
    expect(container.querySelector("input[name=nickname]")).not.toBeNull();
    expect(container.querySelector("input[name=code]")).not.toBeNull();
    expect(container.querySelector("button[name=create]")?.textContent).toBe("Create room");
    expect(container.querySelector("button[name=join]")?.textContent).toBe("Join room");
    expect(container.querySelector("button[name=race]")?.textContent).toBe("Solo race");
    press("race");
    expect(calls.races).toBe(1);
    title.dispose();
    expect(container.childElementCount).toBe(0);
  });

  it("creates a room under the trimmed nickname", () => {
    const { calls, input, press, message } = scene();
    input("nickname").value = "  khiet ";
    press("create");
    expect(calls.created).toEqual(["khiet"]);
    expect(message()).toBe("");
  });

  it("refuses a nickname outside two to twelve characters and says so", () => {
    const { calls, input, press, message } = scene();
    input("nickname").value = "k";
    press("create");
    input("code").value = "BXQF";
    press("join");
    expect(calls.created).toEqual([]);
    expect(calls.joined).toEqual([]);
    expect(message()).toBe("Nickname needs 2 to 12 characters");
    expect(input("nickname").maxLength).toBe(12);
  });

  it("joins with the code in the server's form, typed in any case, on the button or Enter", () => {
    const { calls, input, press } = scene({ nickname: "khiet" });
    input("code").value = "bx qf";
    press("join");
    input("code").value = "BXQF";
    input("code").dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    expect(calls.joined).toEqual([
      ["khiet", "BXQF"],
      ["khiet", "BXQF"],
    ]);
  });

  it("asks for the whole code before joining", () => {
    const { calls, input, press, message } = scene({ nickname: "khiet" });
    input("code").value = "BX";
    press("join");
    expect(calls.joined).toEqual([]);
    expect(message()).toBe("Enter the 4-letter room code");
    expect(input("code").maxLength).toBe(4);
  });

  it("prefills the remembered nickname and the code a link carried, and shows a message", () => {
    const { input, message, title } = scene({
      nickname: "khiet",
      code: "BXQF",
      message: "That room is full",
    });
    expect(input("nickname").value).toBe("khiet");
    expect(input("code").value).toBe("BXQF");
    expect(message()).toBe("That room is full");
    title.message("No room has that code");
    expect(message()).toBe("No room has that code");
  });

  it("offers the three audio toggles as saved, and reports the whole set on each change", () => {
    const { calls, input } = scene({ audio: { sfx: true, engine: false, music: true } });
    const toggle = (name: string) => {
      const box = input(name);
      box.checked = !box.checked;
      box.dispatchEvent(new Event("change", { bubbles: true }));
    };
    expect(input("sfx").type).toBe("checkbox");
    expect(input("sfx").checked).toBe(true);
    expect(input("engine").checked).toBe(false);
    expect(input("music").checked).toBe(true);
    expect(input("sfx").labels?.[0]?.textContent).toBe("Sound effects");
    expect(input("engine").labels?.[0]?.textContent).toBe("Engine");
    expect(input("music").labels?.[0]?.textContent).toBe("Music");
    toggle("music");
    expect(calls.audio).toEqual([{ sfx: true, engine: false, music: false }]);
    toggle("engine");
    expect(calls.audio.at(-1)).toEqual({ sfx: true, engine: true, music: false });
  });
});
