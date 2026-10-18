# ADR 0011: Transport

## Status

Accepted (2026-10-18)

## Context

Inputs and snapshots want unreliable, unordered delivery; room and race
events want reliable delivery. Some networks block UDP. Milestone M0(b),
the WebTransport round trip through real TLS, is the named stack risk.

## Decision

- WebTransport first: inputs and snapshots on datagrams; room, countdown,
  results and all events on a reliable stream. Events are also repeated in
  the next two snapshots and de-duplicated by id.
- WebSocket fallback carries everything on one reliable channel with the
  same message types. The client tries WebTransport and falls back after a 3
  s connect timeout.
- If M0(b) is not done in a weekend, WebSocket becomes the only transport
  for v1.

## Consequences

- The message types (ADR 0004) are transport-agnostic; only the channel
  assignment differs.
- The server terminates its own QUIC with the same certificate Caddy obtains
  (ADR 0015).
- Safari support is best effort because of its WebTransport status.
