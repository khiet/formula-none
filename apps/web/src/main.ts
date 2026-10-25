import { createAudio } from "./audio";
import { loadAudioSettings, saveAudioSettings } from "./audio-settings";
import { type Bridge, loadBridge } from "./bridge";
import { connect, serverUrls } from "./connection";
import type { InstrumentPanel } from "./instrument-panel";
import { loadNickname, saveNickname } from "./nickname";
import { ping } from "./ping";
import type { RaceRun } from "./race-run";
import { createRaceScene, localRaceRun } from "./race-scene";
import { type Gone, keepConnected, type RaceLink } from "./reconnect";
import { createResultsScene, type RaceResults, resultsRows } from "./results-scene";
import { type Countdown, RefusedError, type RoomClient } from "./room-client";
import { roomFromSearch, roomLink, roomSearch } from "./room-link";
import { createRoomScene } from "./room-scene";
import { forgetSession } from "./room-session";
import { createServerRace } from "./server-race";
import { type Seat, takeSeat } from "./take-seat";
import { createTitleScene } from "./title-scene";
import {
  centrelineOf,
  fetchSidecar,
  isTrackName,
  selectTrack,
  TRACKS,
  type TrackFiles,
} from "./track";

/** The connection readout, kept across Race Scenes: each new panel shows the latest status. */
interface ConnectionReadout {
  show(panel: InstrumentPanel): void;
}

/** What the Title says when a race's connection is gone for good (ADR 0005, ADR 0012). */
const RACE_GONE: Readonly<Record<Gone, string>> = {
  refused: "Lost the connection to the server: the race is over",
  replaced: "The race went on in another tab",
};

/**
 * M0(b): reach the server, WebTransport first and WebSocket as the fallback,
 * and show which answered and how fast. The solo race runs locally, so no
 * failure here is fatal.
 */
function reportConnection(bridge: Bridge): ConnectionReadout {
  let status = "connecting";
  let panel: InstrumentPanel | undefined;
  const report = (next: string) => {
    status = next;
    panel?.connection(status);
  };
  (async () => {
    try {
      const connection = await connect({
        ...serverUrls(window.location),
        webTransportTimeoutMs: bridge.webTransportTimeoutMs,
      });
      const nonce = Math.floor(Math.random() * 0x1_0000_0000);
      const rtt = await ping(connection, bridge, nonce, bridge.pingTiming);
      report(`${connection.transport} ${Math.round(rtt)} ms`);
      connection.closed.then(() => report("offline"));
    } catch (error) {
      console.warn("no connection to the server", error);
      report("offline");
    }
  })();
  return {
    show(next) {
      panel = next;
      panel.connection(status);
    },
  };
}

/** Ask the server for a room; its answer is the code to share (ADR 0012). */
async function createRoom(): Promise<string> {
  const response = await fetch("/api/rooms", { method: "POST" });
  if (!response.ok) {
    throw new Error(`could not create a room: ${response.status}`);
  }
  const body = (await response.json()) as { code?: unknown };
  if (typeof body.code !== "string") {
    throw new Error("the server's room has no code");
  }
  return body.code;
}

/** The bytes of a sound file, for the audio to decode once it may start. */
async function fetchBytes(url: string): Promise<ArrayBuffer> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`could not fetch ${url}: ${response.status}`);
  }
  return response.arrayBuffer();
}

/** The track a Countdown names; the default when the client has no such track. */
function trackNamed(name: string): TrackFiles {
  return isTrackName(name) ? TRACKS[name] : selectTrack("");
}

/**
 * Where the game goes next: each Scene's exit names one, as do the steps
 * between Scenes that reach the server or the room.
 */
type Step =
  | { readonly kind: "title"; readonly message?: string; readonly code?: string }
  | { readonly kind: "create"; readonly nickname: string }
  | { readonly kind: "join"; readonly nickname: string; readonly code: string }
  | Seat
  | { readonly kind: "solo" }
  | {
      readonly kind: "results";
      readonly results: RaceResults;
      readonly rematch: Step;
      readonly leave: Step;
    }
  | { readonly kind: "rematch"; readonly client: RoomClient }
  | { readonly kind: "leave"; readonly client: RoomClient };

/** A Scene on the page, shown until disposed; `exit` settles with the first way out taken. */
interface MountedScene {
  readonly exit: Promise<Step>;
  dispose(): void;
}

/**
 * The one rule for a Scene's ways out: the first taken is its exit, and any
 * taken after it, such as a connection closing under a Scene that moved on,
 * does nothing.
 */
function sceneExit(): { exit: Promise<Step>; take(next: Step): void } {
  let take: (next: Step) => void = () => {};
  const exit = new Promise<Step>((resolve) => {
    take = resolve;
  });
  return { exit, take };
}

/** Show `scene` until its exit, then take it off the page. */
async function untilExit(scene: MountedScene): Promise<Step> {
  try {
    return await scene.exit;
  } finally {
    scene.dispose();
  }
}

// The Scenes in order: Title into a room or a solo race, the Room Scene into
// the race on the creator's Start, the race into its results, and the
// results back into the room or another solo race, or the Title. A solo
// race takes its track from `?track=` and runs the sim here; a room's comes
// from the server, which runs the race (ADR 0009).
async function main(container: HTMLElement): Promise<void> {
  const bridge = await loadBridge();
  const connection = reportConnection(bridge);
  // The sound files load now; the AudioContext waits for the first gesture,
  // which the browser's autoplay rule asks for (ADR 0025). Any press or key
  // is one: the Title's buttons on the usual way in, the first key of the
  // race for a player a room link took straight to the room.
  let audioSettings = loadAudioSettings(window.localStorage);
  const audio = createAudio({
    settings: audioSettings,
    topSpeedMetresPerSecond: bridge.cameraTuning.topSpeedMetresPerSecond,
    fetchBytes,
    createContext: () => new AudioContext(),
  });
  const unlockAudio = () => {
    audio.unlock().catch((error: unknown) => console.warn("audio could not start", error));
  };
  for (const gesture of ["pointerdown", "keydown"] as const) {
    window.addEventListener(gesture, unlockAudio);
  }
  const soloTrack = selectTrack(window.location.search);
  const linkedCode = roomFromSearch(window.location.search) ?? "";

  const connectToServer = () =>
    connect({
      ...serverUrls(window.location),
      webTransportTimeoutMs: bridge.webTransportTimeoutMs,
    });

  const mountTitle = ({ message = "", code = "" }: { message?: string; code?: string }) => {
    const { exit, take } = sceneExit();
    const title = createTitleScene(
      container,
      {
        nickname: loadNickname(window.localStorage),
        code,
        message,
        audio: audioSettings,
        nicknameChars: bridge.nicknameChars,
        roomCodeLength: bridge.roomCodeLength,
      },
      {
        create: (nickname) => take({ kind: "create", nickname }),
        join: (nickname, typed) => take({ kind: "join", nickname, code: typed }),
        race: () => take({ kind: "solo" }),
        audio(settings) {
          audioSettings = settings;
          saveAudioSettings(window.localStorage, settings);
          audio.settings(settings);
        },
      },
    );
    return { exit, dispose: title.dispose };
  };

  /** Ask the server for a room and join it, or come back to the Title with why not. */
  const createAndJoin = async (nickname: string): Promise<Step> => {
    try {
      return { kind: "join", nickname, code: await createRoom() };
    } catch (error) {
      console.warn("could not create a room", error);
      return { kind: "title", message: "Could not create a room; is the server up?" };
    }
  };

  /** Take a seat in the room, or come back to the Title with the reason. */
  const enterRoom = async (nickname: string, code: string): Promise<Step> => {
    saveNickname(window.localStorage, nickname);
    let seat: Seat;
    try {
      seat = await takeSeat(
        { storage: window.localStorage, codec: bridge, connect: connectToServer },
        { code, nickname },
      );
    } catch (error) {
      // The server closes a refused connection itself.
      if (error instanceof RefusedError) return { kind: "title", message: error.message, code };
      console.warn("could not join the room", error);
      return { kind: "title", message: "Could not reach the server; try again", code };
    }
    // Refreshing the page comes back to the room.
    window.history.replaceState(null, "", roomSearch(seat.client.room.code));
    return seat;
  };

  /** Leave the room and take its code out of the page's link. */
  const leaveRoom = (client: RoomClient) => {
    client.leave();
    forgetSession(window.localStorage);
    window.history.replaceState(null, "", window.location.pathname);
  };

  const mountRoom = (client: RoomClient): MountedScene => {
    const { exit, take } = sceneExit();
    const scene = createRoomScene(
      container,
      {
        slot: client.slot,
        room: client.room,
        link: roomLink(window.location, client.room.code),
      },
      {
        ready: (ready) => client.ready(ready),
        start: () => client.start(),
        leave: () => take({ kind: "leave", client }),
      },
    );
    const stopRoom = client.onRoom((room) => scene.update(room));
    const stopCountdown = client.onCountdown((countdown) =>
      take({ kind: "race", client, countdown }),
    );
    client.closed.then(() =>
      take({ kind: "title", message: "Lost the connection to the room", code: client.room.code }),
    );
    return {
      exit,
      dispose() {
        stopRoom();
        stopCountdown();
        scene.dispose();
      },
    };
  };

  /**
   * The race a room starts: the server runs it and every client draws its
   * snapshots; the local player's kart is the one their slot has on the
   * grid, and the names come from the room as it stood at the Countdown.
   */
  const mountRoomRace = async (client: RoomClient, countdown: Countdown) => {
    const track = trackNamed(countdown.track);
    const sidecar = await fetchSidecar(track);
    const players = client.room.players;
    const run = createServerRace(bridge, client, countdown, client.slot, sidecar);
    const nickname = (kart: number) =>
      players.find((player) => player.slot === countdown.grid[kart])?.nickname;
    const link = keepConnected(client, {
      timing: bridge.reconnectTiming,
      connect: connectToServer,
    });
    return mountRace(
      track,
      sidecar,
      run,
      nickname,
      { rematch: { kind: "rematch", client }, leave: { kind: "leave", client } },
      link,
    );
  };

  const mountSoloRace = async () => {
    const sidecar = await fetchSidecar(soloTrack);
    const run = localRaceRun(bridge, soloTrack.name, sidecar);
    return mountRace(soloTrack, sidecar, run, () => undefined, {
      rematch: { kind: "solo" },
      leave: { kind: "title" },
    });
  };

  /**
   * Race `run` on `track`, whose `sidecar` draws the course map, to its
   * results, whose Scene goes on to `after`'s steps. A room's race shows the
   * connection banner while `link` reconnects, and goes back to the Title
   * with a message once its connection is gone for good, as a deploy under a
   * race does (ADR 0005, ADR 0012).
   */
  const mountRace = async (
    track: TrackFiles,
    sidecar: string,
    run: RaceRun,
    nickname: (kart: number) => string | undefined,
    after: { readonly rematch: Step; readonly leave: Step },
    link?: RaceLink,
  ): Promise<MountedScene> => {
    const race = await createRaceScene(
      container,
      bridge,
      track,
      centrelineOf(sidecar),
      run,
      nickname,
      audio.race(),
    );
    connection.show(race.panel);
    const { exit, take } = sceneExit();
    link?.onLost((lost) => race.panel.connectionLost(lost));
    link?.gone.then((gone) => take({ kind: "title", message: RACE_GONE[gone] }));
    race.finished.then((finished) =>
      take({ kind: "results", results: { ...finished, nickname }, ...after }),
    );
    return {
      exit,
      dispose() {
        link?.dispose();
        race.dispose();
      },
    };
  };

  const mountResults = (step: Extract<Step, { kind: "results" }>): MountedScene => {
    const { exit, take } = sceneExit();
    const scene = createResultsScene(container, resultsRows(step.results), {
      rematch: () => take(step.rematch),
      leave: () => take(step.leave),
    });
    return { exit, dispose: scene.dispose };
  };

  const next = async (step: Step): Promise<Step> => {
    switch (step.kind) {
      case "title":
        return untilExit(mountTitle(step));
      case "create":
        return createAndJoin(step.nickname);
      case "join":
        return enterRoom(step.nickname, step.code);
      case "room":
        return untilExit(mountRoom(step.client));
      case "race":
        return untilExit(await mountRoomRace(step.client, step.countdown));
      case "solo":
        return untilExit(await mountSoloRace());
      case "results":
        return untilExit(mountResults(step));
      case "rematch":
        step.client.rematch();
        return { kind: "room", client: step.client };
      case "leave":
        leaveRoom(step.client);
        return { kind: "title" };
    }
  };

  // A shared link with a remembered nickname goes straight into the room;
  // without one, the Title asks for the nickname with the code filled in.
  const nickname = loadNickname(window.localStorage);
  let step: Step =
    linkedCode !== "" && nickname !== ""
      ? { kind: "join", nickname, code: linkedCode }
      : { kind: "title", code: linkedCode };
  for (;;) {
    step = await next(step);
  }
}

function fail(error: unknown): void {
  console.error("Formula None failed", error);
}

const app = document.querySelector<HTMLDivElement>("#app");
if (app) {
  main(app).catch(fail);
}
