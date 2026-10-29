# ADR 0049: The server probes each connection

## Status

Accepted (2026-10-29)

## Context

ADR 0018 asks for round-trip time and packet loss per client, measured
from the server. The one Ping the protocol had was the client's: it proves
a transport at connect time (ADR 0011), and the server, which never knows
when the client sent it, cannot time it. Loss could be read off gaps in the
input sequences, but only mid-race and only upstream.

## Decision

- The server probes every seated player once a second: a `Ping` with a
  nonce on the datagram lane, the lane the snapshots and inputs ride, so
  the measurement is of the lane that matters. The client answers at once
  with a `Pong` carrying the nonce on the same lane.
- The round trip of each answered probe goes into the per-client histogram.
  A probe not answered before the next one is sent counts as lost, and a
  late `Pong` changes nothing. The per-client loss gauge is the lost share
  of the last 64 probes, about a minute, which resolves the 2 percent the
  reconciliation is tuned for (ADR 0009).
- The client's one report, `RaceReport`, carries its reconciliation
  corrections per second and is sent when the server's `RaceEnded` event
  arrives; the server records it against the room and seat and logs it.
- Series are labelled by room code and slot. A gauge keeps its last value
  after the room closes; players per room reads zero, the dashboard shows
  rooms above zero, and the server's restart on every deploy clears the
  rest.
- The probe interval and the loss window are tuned in `tuning.rs`
  (ADR 0014).

## Consequences

- Protocol version 12: `ServerMessage::Ping`, `ClientMessage::Pong` and
  `ClientMessage::RaceReport`.
- A client hears from the server at least once a second even between
  races, so the connection banner (ADR 0012) shows only when the server is
  truly unreachable.
- Over the WebSocket fallback the lane is reliable, so loss reads zero
  there; a high round trip is the only sign of a poor connection.
- No telemetry leaves the browser but the echoed nonce and the one number
  at race end, as ADR 0018 requires.
