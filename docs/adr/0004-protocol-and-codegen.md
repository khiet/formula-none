# ADR 0004: Protocol and codegen

## Status

Accepted (2026-10-18)

## Context

Client and server must agree on every message shape. Two hand-written copies
of the types drift.

## Decision

- A Rust crate, `protocol`, defines every message type once and serialises
  with postcard.
- TypeScript types are generated from the Rust definitions with ts-rs into
  the web app and committed.
- A CI check regenerates the types and fails when the output differs from the
  committed files.

## Consequences

- Changing a message means changing Rust, regenerating, and committing both;
  CI catches a forgotten regeneration.
- Encode and decode round-trips are tested for every message type in Rust,
  and the generated TypeScript decodes the same bytes in Vitest.
- Postcard's compact encoding keeps inputs near 8 bytes and snapshots near
  400 bytes at eight karts (ADR 0010).
