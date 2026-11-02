# ADR 0005: Server

## Status

Accepted (2026-10-18). The input handling in its Outcome is superseded by
ADR 0019, and its room count bounded only by CPU by ADR 0073.

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

## Outcome

Landed with issue 17 (2026-10-19).

- The room task in `crates/server/src/rooms.rs` owns the race: on the
  creator's Start it builds `crates/server/src/race.rs` from the room's
  grid and ticks it from a tokio interval at the tick rate, applying the
  newest input each human has sent and sending the snapshots and events.
- Track sidecars are compiled into the binary (`crates/server/src/tracks.rs`),
  so the distroless container needs no asset volume.
- Each race ends with one log line giving its tick count, mean and longest
  tick and how many ticks overran the budget; the metrics of ADR 0018
  replace it later. One such line from a debug build on a laptop read a
  mean of about 1.4 ms per tick of eight karts against the 16.7 ms budget.
- A deploy's shutdown ends every session; a client whose connection ends
  mid-race goes back to the Title Scene with a message.

Restart survival landed with issue 28 (2026-10-30), by ADR 0053: an Open
room is kept as a file on a volume and brought back by the next server,
and the client in the Room Scene rejoins it with its session token.

Measured on the instance with issue 194 (2026-11-02), at commit abac71b: a
`docker compose restart server` to the first healthy answer took 0.84 s,
against the 10 s the Consequences ask for. A room open in a browser showed
"Connection lost: reconnecting" for 1.2 s and came back in the Room Scene with
its seat still Ready. The figure is for a restart of the running image; a
deploy also pulls the image and recreates every container, which was not
measured.
