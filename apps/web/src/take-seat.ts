// Into a room over a new connection (ADR 0012): by the session token kept
// for it, which takes back a seat held mid-race, or else as a new player.

import type { MessageCodec } from "./bridge";
import type { Connection } from "./connection";
import {
  type Countdown,
  type JoinRequest,
  joinRoom,
  RefusedError,
  type RoomClient,
  rejoinRoom,
} from "./room-client";
import { forgetSession, loadSession, saveSession } from "./room-session";

export interface SeatOptions {
  /** Where the session token is kept across a closed tab. */
  readonly storage: Storage;
  readonly codec: MessageCodec;
  /** A new connection to the server; each attempt takes its own. */
  connect(): Promise<Connection>;
}

/** Where the seat puts the player: the Room Scene, or the race a rejoin found under way. */
export type Seat =
  | { readonly kind: "room"; readonly client: RoomClient }
  | { readonly kind: "race"; readonly client: RoomClient; readonly countdown: Countdown };

/**
 * Take a seat in `request.code`'s room and keep its token. A kept token the
 * room refuses is forgotten and the player joins afresh; any other failure,
 * and a refused join, rejects with what went wrong.
 */
export async function takeSeat(options: SeatOptions, request: JoinRequest): Promise<Seat> {
  const client = await seatClient(options, request);
  saveSession(options.storage, { code: client.room.code, token: client.token });
  return client.race === undefined
    ? { kind: "room", client }
    : { kind: "race", client, countdown: client.race };
}

async function seatClient(
  { storage, codec, connect }: SeatOptions,
  request: JoinRequest,
): Promise<RoomClient> {
  const token = loadSession(storage, request.code);
  if (token !== undefined) {
    try {
      return await rejoinRoom(await connect(), codec, { code: request.code, token });
    } catch (error) {
      // The seat went with the race it was kept for; any other refusal
      // the Join below meets too.
      if (!(error instanceof RefusedError)) throw error;
      forgetSession(storage);
    }
  }
  return joinRoom(await connect(), codec, request);
}
