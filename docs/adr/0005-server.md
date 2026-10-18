# ADR 0005: Server

## Status

Accepted (2026-10-18)

## Context

The server is the authority for every kart and every item outcome, runs the
same sim as the browser, and must fit in one small container (ADR 0017).

## Decision

- One Rust binary. tokio for the runtime, axum for HTTP (health, version,
  room create and join), wtransport for race traffic over WebTransport, and a
  WebSocket endpoint carrying the same messages as the fallback (ADR 0011).
- One tokio task per room, running the sim at a fixed 60 Hz tick.
- The server depends on `sim` and `protocol` and on nothing else in the repo.

## Consequences

- Room count is bounded by CPU on one t4g.small; that is acceptable at
  friends scale.
- A deploy restarts the binary and ends any race in progress; rooms in Open
  state must survive a restart in under 10 s.
- Structured logs and metrics (ADR 0018) are emitted from this binary.
