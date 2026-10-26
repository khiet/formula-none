import type { AudioSettings } from "./audio-settings";
import type { NicknameChars } from "./bridge";
import { button } from "./controls";
import { liveryCss } from "./livery";
import { validateNickname } from "./nickname";
import { backdrop, kartSide } from "./poster";
import { normaliseCode } from "./room-link";

export interface TitleScene {
  /** Show a message to the player, such as why a join was refused. */
  message(text: string): void;
  dispose(): void;
}

export interface TitleHandlers {
  /** Create a room and join it under this nickname. */
  create(nickname: string): void;
  /** Join the room with this code under this nickname. */
  join(nickname: string, code: string): void;
  /** Start a solo race on the selected track. */
  race(): void;
  /** The player changed an audio toggle; the whole set as it now stands. */
  audio(settings: AudioSettings): void;
}

export interface TitleOptions {
  /** The remembered nickname, or empty. */
  readonly nickname: string;
  /** The code a shared link carried, or empty. */
  readonly code: string;
  /** A message to show at once, or empty. */
  readonly message: string;
  /** The audio toggles as saved. */
  readonly audio: AudioSettings;
  readonly nicknameChars: NicknameChars;
  readonly roomCodeLength: number;
}

/**
 * The Title Scene: the game's name, the nickname typed once, and the three
 * ways in: create a room, join one by code, or race alone, with the three
 * audio toggles (ADR 0025) under them. The nickname and the code are
 * checked here, so the handlers only ever see valid ones; a refusal from
 * the server comes back through `message`. Every control is addressable by
 * its name.
 */
export function createTitleScene(
  container: HTMLElement,
  options: TitleOptions,
  handlers: TitleHandlers,
): TitleScene {
  const scene = document.createElement("div");
  scene.id = "title-scene";
  // The crossed-out "1" sits beside the heading, not in it, so the game's name reads as it is.
  const logo = document.createElement("div");
  logo.className = "logo";
  const heading = document.createElement("h1");
  const formula = document.createElement("span");
  formula.textContent = "Formula";
  const none = document.createElement("span");
  none.textContent = "None";
  heading.append(formula, " ", none);
  const one = document.createElement("s");
  one.className = "one";
  one.setAttribute("aria-hidden", "true");
  one.textContent = "1";
  logo.append(heading, one);
  const tagline = document.createElement("p");
  tagline.className = "tagline";
  tagline.textContent = "Kart racing for up to 8 friends";

  const nickname = document.createElement("input");
  nickname.name = "nickname";
  nickname.placeholder = "Nickname";
  nickname.maxLength = options.nicknameChars.max;
  nickname.value = options.nickname;
  nickname.setAttribute("aria-label", "Nickname");

  const code = document.createElement("input");
  code.name = "code";
  code.placeholder = "Code";
  code.maxLength = options.roomCodeLength;
  code.value = options.code;
  code.autocapitalize = "characters";
  code.setAttribute("aria-label", "Room code");

  const message = document.createElement("p");
  message.dataset.message = "";
  message.textContent = options.message;

  const show = (text: string) => {
    message.textContent = text;
  };
  /** The nickname as it will be sent, or null after showing why not. */
  const checkedNickname = (): string | null => {
    const check = validateNickname(nickname.value, options.nicknameChars);
    if (!check.ok) {
      show(check.message);
      nickname.focus();
      return null;
    }
    return check.nickname;
  };
  const create = () => {
    const name = checkedNickname();
    if (name !== null) {
      show("");
      handlers.create(name);
    }
  };
  const join = () => {
    const name = checkedNickname();
    if (name === null) return;
    const typed = normaliseCode(code.value);
    if (typed.length !== options.roomCodeLength) {
      show(`Enter the ${options.roomCodeLength}-letter room code`);
      code.focus();
      return;
    }
    show("");
    handlers.join(name, typed);
  };
  code.addEventListener("keydown", (event) => {
    if (event.key === "Enter") join();
  });

  const row = (...children: HTMLElement[]): HTMLDivElement => {
    const element = document.createElement("div");
    element.className = "title-row";
    element.append(...children);
    return element;
  };

  let audio = options.audio;
  const toggle = (name: keyof AudioSettings, text: string): HTMLLabelElement => {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.name = name;
    box.checked = audio[name];
    box.addEventListener("change", () => {
      audio = { ...audio, [name]: box.checked };
      handlers.audio(audio);
    });
    const caption = document.createElement("span");
    caption.textContent = text;
    const label = document.createElement("label");
    label.className = "chip";
    label.append(box, caption);
    return label;
  };
  const toggles = row(
    toggle("sfx", "Sound effects"),
    toggle("engine", "Engine"),
    toggle("music", "Music"),
  );
  toggles.classList.add("audio-toggles");

  const runners = [1, 2].map((slot) => {
    const runner = document.createElement("div");
    runner.className = "runner";
    runner.style.setProperty("--livery", liveryCss(slot));
    runner.append(kartSide());
    return runner;
  });

  scene.append(
    ...backdrop(),
    ...runners,
    logo,
    tagline,
    row(nickname),
    row(button("create", "Create room", create)),
    row(code, button("join", "Join room", join)),
    row(button("race", "Solo race", handlers.race)),
    toggles,
    message,
  );
  container.append(scene);
  // A link lands on the code field with the nickname to fill in; otherwise the nickname comes first.
  (options.code !== "" && options.nickname !== "" ? code : nickname).focus();
  return {
    message: show,
    dispose() {
      scene.remove();
    },
  };
}
