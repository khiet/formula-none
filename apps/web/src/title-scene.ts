import type { AudioSettings } from "./audio-settings";
import type { NicknameChars } from "./bridge";
import { button } from "./controls";
import { type CourseChoice, createCoursePreview } from "./course-preview";
import { flag } from "./flags";
import type { BotLevel, RaceDistance, Tyres } from "./generated/sim";
import { livery } from "./livery";
import { validateNickname } from "./nickname";
import { backdrop, kartSide } from "./poster";
import { normaliseCode } from "./room-link";
import { BOT_LEVELS, type Course, DISTANCE_CHOICES, TYRES_CHOICES } from "./track";

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
  /** Start a solo race on the picked course. */
  race(): void;
  /** The player changed an audio toggle; the whole set as it now stands. */
  audio(settings: AudioSettings): void;
  /** The player picked this course, for the next Create room or Solo race. */
  course(course: Course): void;
  /** The player picked this bot level, for the next Create room or Solo race. */
  botLevel(level: BotLevel): void;
  /** The player picked this for the tyres, for the next Create room or Solo race. */
  tyres(tyres: Tyres): void;
  /** The player picked this race distance, for the next Create room or Solo race. */
  distance(distance: RaceDistance): void;
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
  /** Every course in the picker's order. */
  readonly courses: readonly CourseChoice[];
  /** The course picked when the Scene opens, one of `courses`. */
  readonly course: Course;
  /** The bot level picked when the Scene opens. */
  readonly botLevel: BotLevel;
  /** The Tyres pick when the Scene opens. */
  readonly tyres: Tyres;
  /** The race distance picked when the Scene opens. */
  readonly distance: RaceDistance;
  /** Laps in a race of each distance, for the preview. */
  readonly raceLaps: Readonly<Record<RaceDistance, number>>;
  /** Whether the player asks for reduced motion, which holds the preview's lap dot still. */
  readonly reducedMotion: () => boolean;
}

/** A pick under the flags: a heading, a tile per choice and a line describing the one picked. */
function pickRow<Value extends string>(row: {
  readonly name: string;
  readonly heading: string;
  readonly choices: readonly {
    readonly value: Value;
    readonly name: string;
    readonly blurb: string;
  }[];
  readonly picked: Value;
  onPick(value: Value): void;
}): HTMLElement {
  const fieldset = document.createElement("fieldset");
  fieldset.className = "bots";
  fieldset.setAttribute("aria-label", row.heading);
  const heading = document.createElement("h2");
  heading.textContent = row.heading;
  const tiles = document.createElement("div");
  tiles.className = "bot-tiles";
  const blurb = document.createElement("p");
  blurb.dataset[`${row.name}Blurb`] = "";
  for (const choice of row.choices) {
    const radio = document.createElement("input");
    radio.type = "radio";
    radio.name = row.name;
    radio.value = choice.value;
    radio.checked = choice.value === row.picked;
    radio.addEventListener("change", () => {
      blurb.textContent = `${choice.name}: ${choice.blurb}.`;
      row.onPick(choice.value);
    });
    const caption = document.createElement("span");
    caption.className = "flag-name";
    caption.textContent = choice.name;
    const tile = document.createElement("label");
    tile.className = "flag-tile bot-tile";
    tile.append(radio, caption);
    tiles.append(tile);
    if (radio.checked) blurb.textContent = `${choice.name}: ${choice.blurb}.`;
  }
  fieldset.append(heading, tiles, blurb);
  return fieldset;
}

/**
 * The Title Scene: the game's name, the nickname typed once, and the three
 * ways in: create a room, join one by code, or race alone, with the three
 * audio toggles (ADR 0025) under them; beside them, the course a created
 * room or a solo race runs (ADR 0061), as a flag per course and a preview
 * of the one picked, with the bot level (ADR 0078) and the Tyres pick
 * (ADR 0083) under the flags as a tile per choice and a line describing
 * the one picked. The nickname and
 * the code are checked here, so the handlers only ever see valid ones; a
 * refusal from the server comes back through `message`. Every control is
 * addressable by its name, the flags as radios named `course` with the
 * track's name as their value, the levels as radios named `bots` and the
 * Tyres picks as radios named `tyres` and the race distance picks as radios
 * named `distance`.
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
    // The runners wear the slot's whole livery (ADR 0051): the same car as the podium's and the View's.
    runner.append(kartSide(livery(slot)));
    return runner;
  });

  const wayIn = document.createElement("div");
  wayIn.className = "way-in";
  wayIn.append(
    logo,
    tagline,
    row(nickname),
    row(button("create", "Create room", create)),
    row(code, button("join", "Join room", join)),
    row(button("race", "Solo race", handlers.race)),
    toggles,
    message,
  );

  const courseHeading = document.createElement("div");
  courseHeading.className = "course-heading";
  const courseTitle = document.createElement("h2");
  courseTitle.textContent = "Course";
  const courseNote = document.createElement("p");
  courseNote.textContent = "For create room and solo race";
  courseHeading.append(courseTitle, courseNote);
  // The radios share the name `course`, so the arrow keys move between the flags as a native radio group's do.
  const flags = document.createElement("fieldset");
  flags.className = "flags";
  flags.setAttribute("aria-label", "Course");
  const preview = createCoursePreview({
    laps: options.raceLaps[options.distance],
    reducedMotion: options.reducedMotion,
  });
  for (const choice of options.courses) {
    const radio = document.createElement("input");
    radio.type = "radio";
    radio.name = "course";
    radio.value = choice.course.track;
    radio.checked = choice.course.track === options.course.track;
    radio.addEventListener("change", () => {
      preview.show(choice);
      handlers.course(choice.course);
    });
    const caption = document.createElement("span");
    caption.className = "flag-name";
    caption.textContent = choice.course.name;
    const tile = document.createElement("label");
    tile.className = "flag-tile";
    tile.append(radio, flag(choice.course.flag), caption);
    flags.append(tile);
    if (radio.checked) preview.show(choice);
  }
  // Each pick is a radio group of its own, styled as the flag tiles are.
  const bots = pickRow({
    name: "bots",
    heading: "Bots",
    choices: BOT_LEVELS.map(({ level, name, blurb }) => ({ value: level, name, blurb })),
    picked: options.botLevel,
    onPick: handlers.botLevel,
  });
  const tyres = pickRow({
    name: "tyres",
    heading: "Tyres",
    choices: TYRES_CHOICES.map(({ tyres, name, blurb }) => ({ value: tyres, name, blurb })),
    picked: options.tyres,
    onPick: handlers.tyres,
  });
  const picks = document.createElement("div");
  picks.className = "picks";
  const distance = pickRow({
    name: "distance",
    heading: "Distance",
    choices: DISTANCE_CHOICES.map(({ distance, name, blurb }) => ({
      value: distance,
      name,
      blurb,
    })),
    picked: options.distance,
    onPick(picked) {
      preview.laps(options.raceLaps[picked]);
      handlers.distance(picked);
    },
  });
  picks.append(bots, tyres, distance);
  const courseColumn = document.createElement("div");
  courseColumn.className = "course";
  courseColumn.append(courseHeading, flags, picks, preview.element);

  scene.append(...backdrop(), ...runners, wayIn, courseColumn);
  container.append(scene);
  // A link lands on the code field with the nickname to fill in; otherwise the nickname comes first.
  (options.code !== "" && options.nickname !== "" ? code : nickname).focus();
  return {
    message: show,
    dispose() {
      preview.dispose();
      scene.remove();
    },
  };
}
