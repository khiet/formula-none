import { type Bridge, loadBridge } from "./bridge";
import { type Connection, connect, serverUrls } from "./connection";
import type { InstrumentPanel } from "./instrument-panel";
import { loadNickname, saveNickname } from "./nickname";
import { ping } from "./ping";
import { createRaceScene, type RaceSetup } from "./race-scene";
import { createResultsScene, type RaceResults, resultsRows } from "./results-scene";
import { type Countdown, joinRoom, RefusedError, type RoomClient } from "./room-client";
import { roomFromSearch, roomLink, roomSearch } from "./room-link";
import { createRoomScene } from "./room-scene";
import { createTitleScene } from "./title-scene";
import { fetchSidecar, isTrackName, selectTrack, TRACKS, type TrackFiles } from "./track";

/** The connection readout, kept across Race Scenes: each new panel shows the latest status. */
interface ConnectionReadout {
  show(panel: InstrumentPanel): void;
}

/**
 * M0(b): reach the server, WebTransport first and WebSocket as the fallback,
 * and show which answered and how fast. The race runs locally either way, so
 * no failure here is fatal.
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

/** The track a Countdown names; the default when the client has no such track. */
function trackNamed(name: string): TrackFiles {
  return isTrackName(name) ? TRACKS[name] : selectTrack("");
}

// The Scenes in order: Title into a room or a solo race, the Room Scene into
// the race on the creator's Start, the race into its results, and the
// results back into another race or the Title. A solo race takes its track
// from `?track=`; a room's comes from the server.
async function main(container: HTMLElement): Promise<void> {
  const bridge = await loadBridge();
  const connection = reportConnection(bridge);
  const soloTrack = selectTrack(window.location.search);
  const linkedCode = roomFromSearch(window.location.search) ?? "";

  const showTitle = (message = "", code = "") => {
    const title = createTitleScene(
      container,
      {
        nickname: loadNickname(window.localStorage),
        code,
        message,
        nicknameChars: bridge.nicknameChars,
        roomCodeLength: bridge.roomCodeLength,
      },
      {
        create(nickname) {
          title.dispose();
          createRoom()
            .then((created) => enterRoom(nickname, created))
            .catch((error) => {
              console.warn("could not create a room", error);
              showTitle("Could not create a room; is the server up?");
            });
        },
        join(nickname, typed) {
          title.dispose();
          enterRoom(nickname, typed);
        },
        race() {
          title.dispose();
          showSoloRace().catch(fail);
        },
      },
    );
  };

  /** Join the room and show it, or come back to the Title with the reason. */
  const enterRoom = async (nickname: string, code: string) => {
    saveNickname(window.localStorage, nickname);
    let client: RoomClient;
    let link: Connection | undefined;
    try {
      link = await connect({
        ...serverUrls(window.location),
        webTransportTimeoutMs: bridge.webTransportTimeoutMs,
      });
      client = await joinRoom(link, bridge, { code, nickname });
    } catch (error) {
      // The server closes a refused connection itself; any other failure leaves ours to close.
      link?.close();
      const message =
        error instanceof RefusedError ? error.message : "Could not reach the server; try again";
      if (!(error instanceof RefusedError)) console.warn("could not join the room", error);
      showTitle(message, code);
      return;
    }
    // Refreshing the page comes back to the room.
    window.history.replaceState(null, "", roomSearch(client.room.code));
    showRoom(client);
  };

  const showRoom = (client: RoomClient) => {
    // Cleared when the Scene moves on, so a connection lost later is not
    // reported against a room the player is no longer looking at.
    let waiting = true;
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
        leave() {
          waiting = false;
          client.leave();
          scene.dispose();
          window.history.replaceState(null, "", window.location.pathname);
          showTitle();
        },
      },
    );
    client.onRoom((room) => scene.update(room));
    client.onCountdown((countdown) => {
      waiting = false;
      scene.dispose();
      showRoomRace(client, countdown).catch(fail);
    });
    client.closed.then(() => {
      if (waiting) {
        scene.dispose();
        showTitle("Lost the connection to the room", client.room.code);
      }
    });
  };

  /**
   * The race a room starts: every client builds the same sim from the
   * server's seed with one kart per player in slot order, the local player
   * driving theirs; the server takes over the others with issue 17.
   */
  const showRoomRace = async (client: RoomClient, countdown: Countdown) => {
    const players = client.room.players;
    const setup: RaceSetup = {
      seed: countdown.seed,
      humanCount: players.length,
      localKart: players.findIndex((player) => player.slot === client.slot),
    };
    const nickname = (kart: number) => players[kart]?.nickname;
    await showRace(trackNamed(countdown.track), setup, nickname, () => {
      client.leave();
      window.history.replaceState(null, "", window.location.pathname);
    });
  };

  const showSoloRace = () =>
    showRace(
      soloTrack,
      undefined,
      () => undefined,
      () => {},
    );

  const showRace = async (
    track: TrackFiles,
    setup: RaceSetup | undefined,
    nickname: (kart: number) => string | undefined,
    onLeave: () => void,
  ) => {
    const sidecar = await fetchSidecar(track);
    const race = await createRaceScene(container, bridge, track, sidecar, setup);
    connection.show(race.panel);
    const finished = await race.finished;
    race.dispose();
    const results: RaceResults = { ...finished, nickname };
    const scene = createResultsScene(container, resultsRows(results), {
      rematch() {
        scene.dispose();
        showRace(track, setup, nickname, onLeave).catch(fail);
      },
      leave() {
        scene.dispose();
        onLeave();
        showTitle();
      },
    });
  };

  // A shared link with a remembered nickname goes straight into the room;
  // without one, the Title asks for the nickname with the code filled in.
  const nickname = loadNickname(window.localStorage);
  if (linkedCode !== "" && nickname !== "") {
    await enterRoom(nickname, linkedCode);
  } else {
    showTitle("", linkedCode);
  }
}

function fail(error: unknown): void {
  console.error("Formula None failed", error);
}

const app = document.querySelector<HTMLDivElement>("#app");
if (app) {
  main(app).catch(fail);
}
