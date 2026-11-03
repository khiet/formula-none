// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { courseStats } from "./course-preview";
import type { Room } from "./generated/protocol";
import { CONNECTION_LOST_STATUS, createRoomScene, type RoomHandlers } from "./room-scene";
import { COURSES, type WorldPoint } from "./track";

const ROOM: Room = {
  code: "BXQF",
  track: "goldstone",
  botLevel: "pro",
  tyres: "off",
  state: "open",
  players: [
    { slot: 0, nickname: "khiet", livery: 0, state: "unready", creator: true },
    { slot: 2, nickname: "ada", livery: 2, state: "ready", creator: false },
  ],
};
const LINK = "https://formulanone.sushimafia.com/?room=BXQF";
/** A 50 m square standing in for every course's centreline. */
const SQUARE: WorldPoint[] = [
  [0, 0, 0],
  [50, 0, 0],
  [50, 0, 50],
  [0, 0, 50],
];
const COURSE_CHOICES = COURSES.map((course) => ({ course, centreline: SQUARE }));

function scene(slot: number, room: Room = ROOM) {
  const container = document.createElement("div");
  const calls: string[] = [];
  const handlers: RoomHandlers = {
    ready: (ready) => calls.push(`ready ${ready}`),
    start: () => calls.push("start"),
    leave: () => calls.push("leave"),
  };
  const view = createRoomScene(
    container,
    {
      slot,
      room,
      link: LINK,
      capacity: 8,
      courses: COURSE_CHOICES,
      laps: 3,
      reducedMotion: () => true,
    },
    handlers,
  );
  const button = (name: string) => {
    const element = container.querySelector<HTMLButtonElement>(`button[name=${name}]`);
    if (!element) throw new Error(`no button ${name}`);
    return element;
  };
  const rows = () =>
    [...container.querySelectorAll<HTMLLIElement>("li[data-slot]")].map((row) => ({
      slot: row.dataset.slot,
      state: row.dataset.state,
      // The kart is drawing, not text: only the cells beside it.
      text: [...row.children]
        .filter((cell) => !(cell instanceof SVGElement))
        .map((cell) => cell.textContent)
        .join(""),
      livery: row.querySelector<SVGElement>(".kart.livery")?.dataset.livery,
      badges: [...row.querySelectorAll<HTMLElement>("[data-badge]")].map(
        (badge) => badge.dataset.badge,
      ),
      chip: row.querySelector(".state")?.textContent,
    }));
  const status = () => container.querySelector("[data-room-state]")?.textContent;
  return { container, view, calls, button, rows, status };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

/** Stubs the clipboard with a `writeText` that settles as told. */
function stubClipboard(writeText: (text: string) => Promise<void>) {
  const written: string[] = [];
  vi.stubGlobal("navigator", {
    clipboard: {
      writeText: (text: string) => {
        written.push(text);
        return writeText(text);
      },
    },
  });
  return written;
}

describe("Room Scene", () => {
  it("shows the room's course card, read-only, with its name and four numbers", () => {
    const { container } = scene(2);
    const card = container.querySelector("[data-room-course]");
    expect(card?.querySelector("h3")?.textContent).toBe("Goldstone");
    const stats = Array.from(card?.querySelectorAll(".stats div") ?? [], (box) => [
      box.querySelector("b")?.textContent,
      box.querySelector("small")?.textContent,
    ]);
    const numbers = courseStats(SQUARE);
    expect(stats).toEqual([
      [`${Math.round(numbers.lapM)} m`, "Lap"],
      ["3", "Laps"],
      [`${Math.round(numbers.tightestM)} m`, "Tightest"],
      [`${Math.round(numbers.straightM)} m`, "Straight"],
    ]);
    // Read-only: nothing on the card picks a course.
    expect(card?.querySelector("input, button")).toBeNull();
  });

  it("says the room's bot level under the course card, read-only", () => {
    const { container } = scene(2);
    const card = container.querySelector("[data-room-course]");
    expect(card?.querySelector("[data-room-bots]")?.textContent).toBe("Bots: Pro");
    expect(card?.querySelector("[data-room-tyres]")?.textContent).toBe("Tyres: Off");
    expect(card?.querySelector("input, button")).toBeNull();
  });

  it("leaves the course card out for a track no course is", () => {
    const { container } = scene(2, { ...ROOM, track: "oval" });
    expect(container.querySelector("[data-room-course]")).toBeNull();
  });

  it("shows the code, the player count, the link to share and every player with their livery and state", () => {
    const { container, rows } = scene(2);
    expect(container.querySelector("h1")?.textContent).toBe("Room BXQF");
    expect(container.querySelector("[data-room-code]")?.textContent).toBe("BXQF");
    expect(container.querySelector("[data-room-count]")?.textContent).toBe("2/8");
    const link = container.querySelector<HTMLAnchorElement>("a[data-room-link]");
    expect(link?.href).toBe(LINK);
    expect(link?.textContent).toBe(LINK);
    expect(rows()).toEqual([
      {
        slot: "0",
        state: "unready",
        text: "khiet(creator)Not ready",
        livery: "0",
        badges: ["creator"],
        chip: "Not ready",
      },
      {
        slot: "2",
        state: "ready",
        text: "ada(you)Ready",
        livery: "2",
        badges: ["you"],
        chip: "Ready",
      },
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
    expect(rows()[0]?.badges).toEqual(["you", "creator"]);
    expect(rows()[0]?.text).toBe("ada(you)(creator)Not ready");
    button("leave").click();
    expect(calls).toEqual(["leave"]);
    view.dispose();
    expect(container.childElementCount).toBe(0);
  });

  it("says the room is racing once it leaves Open, and that the race is over at Results", () => {
    const { status, button, view, rows } = scene(0);
    view.update({
      ...ROOM,
      state: "countdown",
      players: ROOM.players.map((player) => ({ ...player, state: "racing" })),
    });
    expect(status()).toBe("Racing");
    expect(button("ready").hidden).toBe(true);
    expect(button("start").disabled).toBe(true);
    // Back from the results before the others: they are still Finished.
    view.update({
      ...ROOM,
      state: "results",
      players: ROOM.players.map((player, index) => ({
        ...player,
        state: index === 0 ? "unready" : "finished",
      })),
    });
    expect(status()).toMatch(/^Race over/);
    expect(rows().map((row) => row.state)).toEqual(["unready", "finished"]);
    expect(button("ready").hidden).toBe(false);
    expect(button("start").disabled).toBe(true);
  });

  it("says the connection is lost while the client reconnects, then shows the room again", () => {
    const { status, button, view } = scene(0);
    view.connectionLost(true);
    expect(status()).toBe(CONNECTION_LOST_STATUS);
    // A player still reaches Leave, the one thing that works offline.
    expect(button("leave").hidden).toBe(false);
    // Back: the room as the server now sends it.
    view.connectionLost(false);
    view.update({
      ...ROOM,
      players: ROOM.players.map((player) => ({ ...player, state: "ready" })),
    });
    expect(status()).toBe("Everyone is ready: press Start");
    expect(button("start").disabled).toBe(false);
  });

  it("copies the link from the chip and says so for about two seconds", async () => {
    vi.useFakeTimers();
    const written = stubClipboard(() => Promise.resolve());
    const { button } = scene(0);
    expect(button("copy").textContent).toBe("Copy link");
    button("copy").click();
    await vi.waitFor(() => expect(button("copy").textContent).toBe("Copied!"));
    expect(written).toEqual([LINK]);
    vi.advanceTimersByTime(2000);
    expect(button("copy").textContent).toBe("Copy link");
  });

  it.each([
    [
      "refuses",
      () => stubClipboard(() => Promise.reject(new DOMException("denied", "NotAllowedError"))),
    ],
    // Plain http to a LAN address has no clipboard at all.
    ["is missing", () => vi.stubGlobal("navigator", {})],
  ])("selects the link for copying by hand when the clipboard %s", async (_, stub) => {
    stub();
    const { button, container } = scene(0);
    container.ownerDocument.body.append(container);
    button("copy").click();
    await vi.waitFor(() => expect(document.getSelection()?.toString()).toBe(LINK));
    expect(button("copy").textContent).toBe("Copy link");
    container.remove();
  });
});
