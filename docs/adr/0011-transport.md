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

## Outcome

M0(b) landed inside the time-box (issue 7, 2026-10-18), so WebTransport
stays the first transport and the fallback decision above stands.

- The server terminates QUIC with wtransport on 443 UDP, reading the
  certificate files Caddy writes to its volume and reloading its TLS
  configuration when they change, so a renewal needs no rebuild or restart.
- The M0(b) round trip is a `Ping` datagram answered by a `Pong`; the
  WebSocket fallback at `/api/ws` carries the same postcard bytes, and the
  client encodes and decodes them through the bridge so the wire format
  exists only in Rust.
- The client's `connect` tries WebTransport for
  `sim::tuning::WEBTRANSPORT_CONNECT_TIMEOUT_S` (3 s) and falls back to the
  WebSocket; both return the same `Connection` interface.
- Every deploy's smoke test performs one WebTransport handshake with the
  deployed certificate and fails the job when it does not round-trip.
