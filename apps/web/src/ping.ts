import type { MessageCodec, PingTiming } from "./bridge";
import type { Connection } from "./connection";

/** The Pong `bytes` carry, or null for bytes that are anything else. */
function pongNonce(codec: MessageCodec, bytes: Uint8Array): number | null {
  try {
    const message = codec.decodeServerMessage(bytes);
    return typeof message === "object" && "pong" in message ? message.pong.nonce : null;
  } catch {
    return null;
  }
}

/**
 * One round trip over `connection`: resolves with the milliseconds from the
 * first Ping until the Pong carrying its nonce came back, or rejects after
 * the timeout. The Ping is sent again every resend interval meanwhile,
 * because a datagram may be lost; the server answers every copy it receives
 * and the first Pong wins. Other messages, and Pongs for other nonces, are
 * left alone.
 */
export function ping(
  connection: Connection,
  codec: MessageCodec,
  nonce: number,
  timing: PingTiming,
  now: () => number = () => performance.now(),
): Promise<number> {
  return new Promise((resolve, reject) => {
    const bytes = codec.encodeClientMessage({ ping: { nonce } });
    const sentAt = now();
    const resend = setInterval(() => connection.send(bytes), timing.resendMs);
    const timer = setTimeout(() => {
      clearInterval(resend);
      reject(new Error(`no pong within ${timing.timeoutMs} ms`));
    }, timing.timeoutMs);
    connection.onMessage((received) => {
      if (pongNonce(codec, received) === nonce) {
        clearInterval(resend);
        clearTimeout(timer);
        resolve(now() - sentAt);
      }
    });
    connection.send(bytes);
  });
}
