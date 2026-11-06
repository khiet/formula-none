import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import type { Bridge } from "../src/bridge";
import type { ServerMessage } from "../src/generated/protocol";
import type { KartReadout as SimKartReadout } from "../src/generated/sim";

// An autopilot for a tab's kart: it reads where the kart stands from the
// snapshots on the page's own WebSocket, decoded through the bridge, and
// presses the arrow keys the way a player would. Black box: nothing in the
// page is reached into.

const TRACK_SIDECAR = new URL(
  "../../../assets/tracks/bell-deer/bell-deer.track.json",
  import.meta.url,
);

/** Metres ahead on the centreline the autopilot steers toward, right of the line. */
const LOOKAHEAD_M = 12;
const RIGHT_OF_CENTRE_M = 2.5;
/** Steer this far off the kart's heading before a key goes down. */
const STEER_DEADBAND = 0.12;
/** Lift off above this speed when the line ahead bends more than the angle. */
const CORNER_LOOKAHEAD_M = 28;
const CORNER_BEND_RAD = 0.45;
const CORNER_SPEED_M_PER_S = 17;

export interface Centreline {
  pointAt(progress: number): [number, number];
}

/** The sidecar's centreline as a loop of points with their cumulative length. */
export function centreline(): Centreline {
  const sidecar = JSON.parse(readFileSync(TRACK_SIDECAR, "utf8")) as {
    centreline: [number, number, number][];
  };
  const points = sidecar.centreline.map(([x, , z]) => [x, z] as [number, number]);
  const lengths = [0];
  for (let index = 0; index < points.length; index++) {
    const from = points[index] as [number, number];
    const to = points[(index + 1) % points.length] as [number, number];
    lengths.push((lengths[index] as number) + Math.hypot(to[0] - from[0], to[1] - from[1]));
  }
  const total = lengths[points.length] as number;
  return {
    pointAt(progress) {
      const along = ((progress % total) + total) % total;
      let index = 0;
      while ((lengths[index + 1] as number) < along) index++;
      const from = points[index] as [number, number];
      const to = points[(index + 1) % points.length] as [number, number];
      const segment = (lengths[index + 1] as number) - (lengths[index] as number);
      const t = segment === 0 ? 0 : (along - (lengths[index] as number)) / segment;
      return [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t];
    },
  };
}

/** Yaw about +Y from the quaternion [x, y, z, w]; zero faces -Z. */
function yaw([x, y, z, w]: readonly [number, number, number, number]): number {
  return Math.atan2(2 * (w * y + x * z), 1 - 2 * (y * y + z * z));
}

export interface Controls {
  readonly steer: -1 | 0 | 1;
  readonly throttle: boolean;
  /** Backing up out of a stall. */
  readonly reverse: boolean;
}

/** Under this speed the kart counts as stalled, held by a wall or a kart. */
const STALL_SPEED_M_PER_S = 1;
/** Ticks stalled before the autopilot backs up, and ticks it backs up for. */
const STALL_TICKS = 60;
const REVERSE_TICKS = 60;

/** Where to steer and whether to keep the throttle, from the kart's snapshot. */
export function decide(line: Centreline, kart: SimKartReadout): Controls {
  const progress = kart.splineProgress;
  const ahead = line.pointAt(progress + LOOKAHEAD_M);
  const further = line.pointAt(progress + LOOKAHEAD_M + 1);
  const travel = Math.hypot(further[0] - ahead[0], further[1] - ahead[1]) || Number.EPSILON;
  const target = [
    ahead[0] - ((further[1] - ahead[1]) / travel) * RIGHT_OF_CENTRE_M,
    ahead[1] + ((further[0] - ahead[0]) / travel) * RIGHT_OF_CENTRE_M,
  ];
  const dx = (target[0] as number) - kart.position[0];
  const dz = (target[1] as number) - kart.position[2];
  const length = Math.hypot(dx, dz) || Number.EPSILON;
  const heading = yaw(kart.rotation);
  const rightward = (dx * Math.cos(heading) - dz * Math.sin(heading)) / length;
  const steer = rightward > STEER_DEADBAND ? 1 : rightward < -STEER_DEADBAND ? -1 : 0;

  const bendFrom = line.pointAt(progress + LOOKAHEAD_M);
  const bendMid = line.pointAt(progress + CORNER_LOOKAHEAD_M);
  const bendTo = line.pointAt(progress + CORNER_LOOKAHEAD_M + 1);
  const a = Math.atan2(bendMid[0] - bendFrom[0], bendMid[1] - bendFrom[1]);
  const b = Math.atan2(bendTo[0] - bendMid[0], bendTo[1] - bendMid[1]);
  const bend = Math.abs(Math.atan2(Math.sin(b - a), Math.cos(b - a)));
  const speed = Math.hypot(...kart.velocity);
  const throttle = !(bend > CORNER_BEND_RAD && speed > CORNER_SPEED_M_PER_S);
  return { steer, throttle, reverse: false };
}

/**
 * Drive the page's kart from the snapshots on its WebSocket: the Joined
 * says the slot, the Countdown which kart that is, and every snapshot
 * where it stands. Arrow keys are pressed and released as the decision
 * changes.
 */
export interface Autopilot {
  laps(): number;
  /** The newest snapshot of the kart and the decision taken on it. */
  last(): { kart: SimKartReadout; controls: Controls } | undefined;
}

export function autopilot(page: Page, bridge: Bridge, line: Centreline): Autopilot {
  let slot = -1;
  let kart = -1;
  let laps = 0;
  let last: { kart: SimKartReadout; controls: Controls } | undefined;
  let stalledTicks = 0;
  let reversingUntil = 0;
  const keys: Controls = { steer: 0, throttle: false, reverse: false };
  let held = { ...keys };
  const apply = async (next: Controls) => {
    if (next.throttle !== held.throttle) {
      await (next.throttle ? page.keyboard.down("ArrowUp") : page.keyboard.up("ArrowUp"));
    }
    if (next.reverse !== held.reverse) {
      await (next.reverse ? page.keyboard.down("ArrowDown") : page.keyboard.up("ArrowDown"));
    }
    if (next.steer !== held.steer) {
      if (held.steer === 1) await page.keyboard.up("ArrowRight");
      if (held.steer === -1) await page.keyboard.up("ArrowLeft");
      if (next.steer === 1) await page.keyboard.down("ArrowRight");
      if (next.steer === -1) await page.keyboard.down("ArrowLeft");
    }
    held = { ...next };
  };
  let applying = Promise.resolve();
  page.on("websocket", (socket) => {
    socket.on("framereceived", ({ payload }) => {
      if (typeof payload === "string") return;
      let message: ServerMessage;
      try {
        message = bridge.decodeServerMessage(new Uint8Array(payload));
      } catch {
        return;
      }
      if (typeof message === "string") return;
      if ("joined" in message) {
        slot = message.joined.slot;
      } else if ("countdown" in message) {
        kart = message.countdown.grid.indexOf(slot);
      } else if ("events" in message) {
        for (const { kind } of message.events) {
          if (typeof kind === "string") continue;
          if ("lapCompleted" in kind && kind.lapCompleted.kart === kart) laps += 1;
        }
      } else if ("snapshot" in message && kart >= 0) {
        const mine = message.snapshot.karts[kart];
        if (mine === undefined) return;
        // Throttle before the start window would stall the kart.
        const { tick } = message.snapshot;
        const countingDown = tick < bridge.countdownSeconds * bridge.ticksPerSecond;
        let next: Controls = countingDown
          ? { steer: 0, throttle: false, reverse: false }
          : decide(line, mine);
        // Held by a wall: back up the other way for a while, as a bot does.
        const stalled = !countingDown && Math.hypot(...mine.velocity) < STALL_SPEED_M_PER_S;
        stalledTicks = stalled ? stalledTicks + 1 : 0;
        if (stalledTicks > STALL_TICKS && tick >= reversingUntil) {
          reversingUntil = tick + REVERSE_TICKS;
          stalledTicks = 0;
        }
        if (tick < reversingUntil) {
          next = { steer: -next.steer as -1 | 0 | 1, throttle: false, reverse: true };
        }
        last = { kart: mine, controls: next };
        applying = applying.then(() => apply(next)).catch(() => {});
      }
    });
  });
  return { laps: () => laps, last: () => last };
}
