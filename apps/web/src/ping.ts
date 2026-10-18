import type { Connection } from "./connection";

/** The protocol's Ping and Pong, as the bridge encodes and decodes them. */
export interface PingCodec {
  encodePing(nonce: number): Uint8Array;
  /** Throws for bytes that are not a Pong. */
  decodePong(bytes: Uint8Array): number;
}

/**
 * One round trip over `connection`: resolves with the milliseconds until the
 * Pong carrying this Ping's nonce came back, or rejects after `timeoutMs`.
 * Other messages, and Pongs for other nonces, are left alone.
 */
export function ping(
  connection: Connection,
  codec: PingCodec,
  nonce: number,
  timeoutMs: number,
  now: () => number = () => performance.now(),
): Promise<number> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no pong within ${timeoutMs} ms`)), timeoutMs);
    connection.onMessage((bytes) => {
      let answered: number;
      try {
        answered = codec.decodePong(bytes);
      } catch {
        return;
      }
      if (answered === nonce) {
        clearTimeout(timer);
        resolve(now() - sentAt);
      }
    });
    const sentAt = now();
    connection.send(codec.encodePing(nonce));
  });
}
