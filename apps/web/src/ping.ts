import type { PingTiming } from "./bridge";
import type { Connection } from "./connection";

/** The protocol's Ping and Pong, as the bridge encodes and decodes them. */
export interface PingCodec {
  encodePing(nonce: number): Uint8Array;
  /** Throws for bytes that are not a Pong. */
  decodePong(bytes: Uint8Array): number;
}

/**
 * One round trip over `connection`: resolves with the milliseconds from the
 * first Ping until the Pong carrying its nonce came back, or rejects after
 * the timeout. The Ping is sent again every resend interval meanwhile,
 * because a datagram may be lost; a WebSocket answers each copy and the
 * first Pong wins. Other messages, and Pongs for other nonces, are left alone.
 */
export function ping(
  connection: Connection,
  codec: PingCodec,
  nonce: number,
  timing: PingTiming,
  now: () => number = () => performance.now(),
): Promise<number> {
  return new Promise((resolve, reject) => {
    const bytes = codec.encodePing(nonce);
    const sentAt = now();
    const resend = setInterval(() => connection.send(bytes), timing.resendMs);
    const timer = setTimeout(() => {
      clearInterval(resend);
      reject(new Error(`no pong within ${timing.timeoutMs} ms`));
    }, timing.timeoutMs);
    connection.onMessage((received) => {
      let answered: number;
      try {
        answered = codec.decodePong(received);
      } catch {
        return;
      }
      if (answered === nonce) {
        clearInterval(resend);
        clearTimeout(timer);
        resolve(now() - sentAt);
      }
    });
    connection.send(bytes);
  });
}
