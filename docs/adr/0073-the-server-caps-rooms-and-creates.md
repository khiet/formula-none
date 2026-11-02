# ADR 0073: The server caps rooms and room creation

## Status

Accepted (2026-11-02)

## Context

ADR 0005 accepts a room count bounded by the CPU of one t4g.small. That holds
among friends. Once the repository is public, strangers can find the hostname,
and each room creation is free compute on that instance.

## Decision

- The server hosts at most `ROOMS_MAX` rooms. A create past that is refused with
  503 until a room closes. Rooms a restart brings back are never refused but
  count toward the cap.
- One address may create `ROOM_CREATES_PER_ADDRESS_MAX` rooms per
  `ROOM_CREATE_WINDOW_S`, a sliding window. A create past that is refused with
  429. Only a room actually opened counts. An IPv6 address counts by its /64,
  which one subscriber commonly holds whole.
- The address is the last entry of `X-Forwarded-For`. Caddy sets that header and
  replaces any value the client sent, since it trusts no proxy, and the server's
  HTTP port is reachable only through Caddy.
- A request without the header did not come through Caddy, so only the cap
  applies to it. The Vite dev server and the Playwright suite are such callers.
- The Title Scene shows a message for each refusal, told apart by status.
- All three constants live in `crates/sim/src/tuning.rs` (ADR 0014).

## Consequences

- `ROOMS_MAX` is a guess, not a measurement of what the instance can race.
- Adding `trusted_proxies` to the Caddyfile, or publishing the server's HTTP
  port, would let a client choose its own address and escape the limit.
- A household or office behind one address shares one allowance.
- The limit does not cover WebSocket or WebTransport connections, which can
  still be opened without a room.
- The limit's memory is bounded by the rooms a window can open, so it cannot
  grow without limit under a flood of refused creates.
