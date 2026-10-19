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

## Outcome

Landed with issue 15 (2026-10-19).

- `cargo run -p protocol --example generate` (`pnpm generate:protocol`)
  writes one TypeScript file per message type with ts-rs, the
  `PROTOCOL_VERSION` constant, an index and the sample messages as bytes
  next to their JSON, to `apps/web/src/generated/protocol`, which is
  committed. The web job in CI regenerates and fails on a diff.
- Every enum is externally tagged, because postcard cannot decode serde's
  other taggings; the variant order and field order are the wire format.
- The bridge carries messages across the wasm boundary as JavaScript values
  in the generated shape through serde-wasm-bindgen, so the web app never
  sees postcard bytes other than to send and receive them.
- At eight karts the busy sample snapshot is 458 bytes and an input late in
  a race 9 bytes; the tests hold both within a quarter of the budgets.
