import { button } from "./controls";
import type { Player, Room } from "./generated/protocol";
import { liveryCss } from "./livery";

export interface RoomScene {
  /** Show the room as it now stands. */
  update(room: Room): void;
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
}

/** What a player's row says about them. */
const STATE_LABELS: Readonly<Record<Player["state"], string>> = {
  joining: "Joining",
  unready: "Not ready",
  ready: "Ready",
  racing: "Racing",
  finished: "Finished",
  disconnected: "Disconnected",
};

/**
 * The Room Scene (ADR 0012): the code and link to share, every player with
 * their livery and whether they are ready, the local player's ready toggle,
 * the creator's Start, enabled once every player is ready, and Leave. Each
 * row is addressable by `data-slot` and each button by its name.
 */
export function createRoomScene(
  container: HTMLElement,
  view: RoomView,
  handlers: RoomHandlers,
): RoomScene {
  const scene = document.createElement("div");
  scene.id = "room-scene";
  const heading = document.createElement("h1");
  heading.append("Room ");
  const code = document.createElement("span");
  code.dataset.roomCode = "";
  heading.append(code);
  const link = document.createElement("a");
  link.dataset.roomLink = "";
  link.href = view.link;
  link.textContent = view.link;
  const status = document.createElement("p");
  status.dataset.roomState = "";
  const players = document.createElement("ul");
  players.className = "players";

  let current = view.room;
  const me = (): Player | undefined => current.players.find((player) => player.slot === view.slot);
  const ready = button("ready", "Ready", () => handlers.ready(me()?.state !== "ready"));
  const start = button("start", "Start", handlers.start);
  const leave = button("leave", "Leave", handlers.leave);

  const render = () => {
    code.textContent = current.code;
    players.replaceChildren(
      ...current.players.map((player) => {
        const row = document.createElement("li");
        row.dataset.slot = String(player.slot);
        row.dataset.state = player.state;
        const swatch = document.createElement("span");
        swatch.className = "livery";
        swatch.dataset.livery = String(player.livery);
        swatch.style.background = liveryCss(player.livery);
        const name = document.createElement("span");
        name.className = "nickname";
        name.textContent = player.nickname;
        const tags = [player.slot === view.slot ? "you" : "", player.creator ? "creator" : ""]
          .filter((tag) => tag !== "")
          .map((tag) => `(${tag})`);
        const state = document.createElement("span");
        state.className = "state";
        state.textContent = [...tags, STATE_LABELS[player.state]].join(" ");
        row.append(swatch, name, state);
        return row;
      }),
    );
    const local = me();
    const everyoneReady = current.players.every((player) => player.state === "ready");
    // Mirrors the server's rule: between races, once every player is ready.
    const canStart = everyoneReady && (current.state === "open" || current.state === "results");
    ready.textContent = local?.state === "ready" ? "Unready" : "Ready";
    ready.hidden = !(local?.state === "ready" || local?.state === "unready");
    start.hidden = local?.creator !== true;
    start.disabled = !canStart;
    status.textContent =
      current.state !== "open"
        ? "Starting"
        : everyoneReady
          ? local?.creator
            ? "Everyone is ready: press Start"
            : "Everyone is ready: waiting for the creator to start"
          : "Waiting for everyone to be ready";
  };
  render();

  const buttons = document.createElement("div");
  buttons.className = "room-buttons";
  buttons.append(ready, start, leave);
  scene.append(heading, link, players, status, buttons);
  container.append(scene);
  return {
    update(room) {
      current = room;
      render();
    },
    dispose() {
      scene.remove();
    },
  };
}
