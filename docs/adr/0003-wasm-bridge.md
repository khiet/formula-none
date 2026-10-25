# ADR 0003: WebAssembly bridge

## Status

Accepted (2026-10-18). The consequence that a value the renderer needs
extends the flat buffer is superseded by ADR 0026.

## Context

The browser must run the sim for prediction of the player's own kart, and
Three.js must read kart transforms every frame. Crossing the wasm boundary
per kart per field would be slow and would leak sim types into the client.

## Decision

- A thin `bridge` crate built with wasm-bindgen exposes four operations:
  create, step, apply snapshot, and read kart transforms as a flat buffer.
- One TypeScript wrapper module is the only code that touches the wasm
  exports. Three.js reads transforms from the wrapper and nothing else.
- The bridge holds no game rules; it adapts `sim` and `protocol` to the
  browser.

## Consequences

- The web app depends on the bridge and on nothing else in the Cargo
  workspace (ADR 0006).
- Adding a value the renderer needs means extending the flat buffer and the
  wrapper, not exposing a new sim type.
- The bridge is where wasm build and size problems surface first.
