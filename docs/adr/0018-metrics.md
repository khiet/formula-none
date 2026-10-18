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
