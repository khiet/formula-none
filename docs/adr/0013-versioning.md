# ADR 0013: Versioning

## Status

Accepted (2026-10-18)

## Context

A deploy can change the protocol while a player has an old client loaded in
a tab.

## Decision

- The client sends its protocol version on connect.
- A mismatch is refused with a message telling the player to reload.

## Consequences

- The protocol version is a constant in the `protocol` crate and reaches the
  client through the generated types (ADR 0004).
- No compatibility shims between protocol versions at v1.
