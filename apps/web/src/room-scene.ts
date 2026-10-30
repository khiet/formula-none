import { button } from "./controls";
import type { Player, Room } from "./generated/protocol";
import { livery } from "./livery";
import { backdrop, kartSide } from "./poster";

export interface RoomScene {
  /** Show the room as it now stands. */
  update(room: Room): void;
  /**
   * Say the connection is lost and being remade (ADR 0053) in place of the
   * status line, or stop saying so; the room shown stays as it last stood.
   */
  connectionLost(lost: boolean): void;
  dispose(): void;
}

export interface RoomHandlers {
  ready(ready: boolean): void;
  start(): void;
  leave(): void;
}

export interface RoomView {
  /** The local player's slot, which marks their row and decides whose buttons show. */
  readonly slot: number;
  readonly room: Room;
  /** The link that joins this room, shown for sharing. */
  readonly link: string;
  /** How many players a room holds, for the count beside the code. */
  readonly capacity: number;
}

const COPIED_MS = 2000;

/** The status line while the client rejoins the room over a new connection, as the race's banner reads. */
export const CONNECTION_LOST_STATUS = "Connection lost: reconnecting";

/** What a player's row says about them. */
const STATE_LABELS: Readonly<Record<Player["state"], string>> = {
  joining: "Joining",
  unready: "Not ready",
  ready: "Ready",
  racing: "Racing",
  finished: "Finished",
  disconnected: "Disconnected",
};

/** What the room's state line says. */
export function roomStatus(state: Room["state"], everyoneReady: boolean, creator: boolean): string {
  switch (state) {
    case "countdown":
    case "racing":
      return "Racing";
    case "results":
      return "Race over: everyone back from the results readies up for the next";
    case "closing":
      return "This room has closed";
    case "open":
      return everyoneReady
        ? creator
          ? "Everyone is ready: press Start"
          : "Everyone is ready: waiting for the creator to start"
        : "Waiting for everyone to be ready";
  }
}

/** A "You" or "Creator" badge; the parentheses are hidden, so the row's text still reads "(you)". */
function badge(name: string): HTMLSpanElement {
  const element = document.createElement("span");
  element.dataset.badge = name;
  const paren = (text: string) => {
    const span = document.createElement("span");
    span.className = "paren";
    span.textContent = text;
    return span;
  };
  element.append(paren("("), name, paren(")"));
  return element;
}

/** A joined player as a kart in their livery on the grid, with their nickname, badges and state chip. */
function playerItem(player: Player, you: boolean): HTMLLIElement {
  const row = document.createElement("li");
  row.dataset.slot = String(player.slot);
  row.dataset.state = player.state;
  const kart = kartSide(livery(player.livery));
  kart.classList.add("livery");
  kart.dataset.livery = String(player.livery);
  const name = document.createElement("span");
  name.className = "nickname";
  name.textContent = player.nickname;
  const badges = document.createElement("span");
  badges.className = "badges";
  if (you) badges.append(badge("you"));
  if (player.creator) badges.append(badge("creator"));
  const state = document.createElement("span");
  state.className = "state";
  state.textContent = STATE_LABELS[player.state];
  row.append(kart, name, badges, state);
  return row;
}

/**
 * The Room Scene (ADR 0012) on the Grand Prix poster (ADR 0039): the code
 * and link to share, with a chip that copies the link, every joined player
 * as a kart in their livery on a two-column grid with whether they are
 * ready, the local player's ready toggle, the creator's Start, enabled once
 * every player is ready, and Leave. Each row is addressable by `data-slot`
 * and each button by its name.
 */
export function createRoomScene(
  container: HTMLElement,
  view: RoomView,
  handlers: RoomHandlers,
): RoomScene {
  const scene = document.createElement("div");
  scene.id = "room-scene";
  const header = document.createElement("header");
  const heading = document.createElement("h1");
  heading.append("Room ");
  const code = document.createElement("span");
  code.dataset.roomCode = "";
  heading.append(code);
  const count = document.createElement("span");
  count.dataset.roomCount = "";
  header.append(heading, count);

  const link = document.createElement("a");
  link.dataset.roomLink = "";
  link.href = view.link;
  link.textContent = view.link;
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  const copy = button("copy", "Copy link", () => {
    // Without a clipboard (plain http off localhost) or with it refused, the
    // link is selected so the player can copy it by hand.
    const selectLink = () => {
      clearTimeout(copiedTimer);
      copy.textContent = "Copy link";
      document.getSelection()?.selectAllChildren(link);
    };
    const clipboard = navigator.clipboard as Clipboard | undefined;
    if (clipboard === undefined) {
      selectLink();
      return;
    }
    clipboard.writeText(view.link).then(() => {
      copy.textContent = "Copied!";
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => {
        copy.textContent = "Copy link";
      }, COPIED_MS);
    }, selectLink);
  });
  const share = document.createElement("div");
  share.className = "share";
  share.append(link, copy);

  const status = document.createElement("p");
  status.dataset.roomState = "";
  const players = document.createElement("ul");
  players.className = "players";

  let current = view.room;
  let lost = false;
  const me = (): Player | undefined => current.players.find((player) => player.slot === view.slot);
  const ready = button("ready", "Ready", () => handlers.ready(me()?.state !== "ready"));
  const start = button("start", "Start", handlers.start);
  const leave = button("leave", "Leave", handlers.leave);

  const render = () => {
    code.textContent = current.code;
    count.textContent = `${current.players.length}/${view.capacity}`;
    players.replaceChildren(
      ...current.players.map((player) => playerItem(player, player.slot === view.slot)),
    );
    const local = me();
    const everyoneReady = current.players.every((player) => player.state === "ready");
    // Mirrors the server's rule: between races, once every player is ready.
    const canStart = everyoneReady && (current.state === "open" || current.state === "results");
    ready.textContent = local?.state === "ready" ? "Unready" : "Ready";
    ready.hidden = !(local?.state === "ready" || local?.state === "unready");
    start.hidden = local?.creator !== true;
    start.disabled = !canStart;
    status.textContent = lost
      ? CONNECTION_LOST_STATUS
      : roomStatus(current.state, everyoneReady, local?.creator === true);
  };
  render();

  const buttons = document.createElement("div");
  buttons.className = "room-buttons";
  buttons.append(ready, start, leave);
  scene.append(...backdrop(), header, share, players, status, buttons);
  container.append(scene);
  return {
    update(room) {
      current = room;
      render();
    },
    connectionLost(next) {
      lost = next;
      render();
    },
    dispose() {
      clearTimeout(copiedTimer);
      scene.remove();
    },
  };
}
