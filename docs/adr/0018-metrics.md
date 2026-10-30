# ADR 0018: Metrics

## Status

Accepted (2026-10-18)

## Context

Whether the netcode works is a measurement, not a feeling. The developer
needs to see tick cost, load, latency, loss and correction rate per race.

## Decision

- The server emits metrics with the OpenTelemetry metrics SDK, scraped by
  Prometheus in the same Compose stack (ADR 0015).
- Five metrics: tick duration histogram, players per room gauge, round-trip
  time per client histogram, packet loss per client gauge, and
  reconciliation corrections per second per client (reported by the client
  at race end and logged by the server).
- One Grafana dashboard with these five panels, reached through an SSM port
  forward.
- Structured server logs go to CloudWatch Logs with 14-day retention.

## Consequences

- The client reports its correction count at race end; no other telemetry
  leaves the browser, so there is no tracking.
- Metrics are read by the developer only; nothing is public.

## Outcome

The five metrics and the dashboard landed with issue 27; the server's
probes that feed them are ADR 0049.

Structured logs landed with issue 28 (2026-10-30). The server writes one
JSON object per line to stdout (`crates/server/src/log.rs`): a `ts`, an
`event`, and `room`, `slot` and `tick` wherever the line is about a room,
a player or a moment in a race. The awslogs driver in `deploy/compose.yaml`
ships the lines to the `/formula-none/server` log group, which `infra/`
keeps for 14 days, and CloudWatch Logs Insights reads the fields by name;
the query that finds a room's lines is in `deploy/README.md`.
