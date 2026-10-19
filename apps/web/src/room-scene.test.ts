// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Room } from "./generated/protocol";
import { createRoomScene, type RoomHandlers } from "./room-scene";

const ROOM: Room = {
  code: "BXQF",
  state: "open",
  players: [
    { slot: 0, nickname: "khiet", livery: 0, state: "unready", creator: true },
    { slot: 2, nickname: "ada", livery: 2, state: "ready", creator: false },
  ],
};
const LINK = "https://formulanone.sushimafia.com/?room=BXQF";

function scene(slot: number, room: Room = ROOM) {
  const container = document.createElement("div");
  const calls: string[] = [];
  const handlers: RoomHandlers = {
    ready: (ready) => calls.push(`ready ${ready}`),
    start: () => calls.push("start"),
    leave: () => calls.push("leave"),
  };
  const view = createRoomScene(container, { slot, room, link: LINK }, handlers);
  const button = (name: string) => {
    const element = container.querySelector<HTMLButtonElement>(`button[name=${name}]`);
    if (!element) throw new Error(`no button ${name}`);
    return element;
  };
  const rows = () =>
    [...container.querySelectorAll<HTMLLIElement>("li[data-slot]")].map((row) => ({
      slot: row.dataset.slot,
      state: row.dataset.state,
      text: row.textContent,
      livery: row.querySelector<HTMLElement>(".livery")?.dataset.livery,
    }));
  const status = () => container.querySelector("[data-room-state]")?.textContent;
  return { container, view, calls, button, rows, status };
}

describe("Room Scene", () => {
  it("shows the code, the link to share and every player with their livery and state", () => {
    const { container, rows } = scene(2);
    expect(container.querySelector("[data-room-code]")?.textContent).toBe("BXQF");
    const link = container.querySelector<HTMLAnchorElement>("a[data-room-link]");
    expect(link?.href).toBe(LINK);
    expect(link?.textContent).toBe(LINK);
    expect(rows()).toEqual([
      {
        slot: "0",
        state: "unready",
        text: "khiet(creator) Not ready",
        livery: "0",
      },
      { slot: "2", state: "ready", text: "ada(you) Ready", livery: "2" },
    ]);
  });

  it("toggles the local player's ready state from the button", () => {
    const { calls, button, view } = scene(0);
    expect(button("ready").textContent).toBe("Ready");
    button("ready").click();
    expect(calls).toEqual(["ready true"]);
    view.update({
      ...ROOM,
      players: ROOM.players.map((player) => ({ ...player, state: "ready" })),
    });
    expect(button("ready").textContent).toBe("Unready");
    button("ready").click();
    expect(calls).toEqual(["ready true", "ready false"]);
  });

  it("shows Start to the creator only, enabled once everyone is ready", () => {
    const creator = scene(0);
    expect(creator.button("start").hidden).toBe(false);
    expect(creator.button("start").disabled).toBe(true);
    expect(creator.status()).toBe("Waiting for everyone to be ready");
    const everyoneReady: Room = {
      ...ROOM,
      players: ROOM.players.map((player) => ({ ...player, state: "ready" })),
    };
    creator.view.update(everyoneReady);
    expect(creator.button("start").disabled).toBe(false);
    expect(creator.status()).toBe("Everyone is ready: press Start");
    creator.button("start").click();
    expect(creator.calls).toEqual(["start"]);

    const friend = scene(2, everyoneReady);
    expect(friend.button("start").hidden).toBe(true);
    expect(friend.status()).toBe("Everyone is ready: waiting for the creator to start");
  });

  it("follows the creator role as it passes, and leaves on the button", () => {
    const { button, view, calls, rows, container } = scene(2);
    view.update({
      ...ROOM,
      players: [{ slot: 2, nickname: "ada", livery: 2, state: "unready", creator: true }],
    });
    expect(button("start").hidden).toBe(false);
    expect(rows()[0]?.text).toBe("ada(you) (creator) Not ready");
    button("leave").click();
    expect(calls).toEqual(["leave"]);
    view.dispose();
    expect(container.childElementCount).toBe(0);
  });

  it("says the race is starting once the room leaves Open", () => {
    const { status, button, view } = scene(0);
    view.update({
      ...ROOM,
      state: "countdown",
      players: ROOM.players.map((player) => ({ ...player, state: "racing" })),
    });
    expect(status()).toBe("Starting");
    expect(button("ready").hidden).toBe(true);
  });
});
