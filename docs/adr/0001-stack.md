# ADR 0001: Stack

## Status

Accepted (2026-10-18)

## Context

The project exists to learn four things, in this tie-break order when they
conflict: ship something friends play, game architecture and netcode, agentic
workflow, Rust, hosting and operations on AWS. The previous project stalled
when the stack's hard problem had no fallback. A Bevy end-to-end stack was
considered and rejected because the frontend would stop being TypeScript and
Bevy's web path is rougher than Three.js. Tauri was rejected for v1 because it
wraps a web client and does nothing for a hosted multiplayer server.

## Decision

- Client: TypeScript in strict mode, Vite, Three.js with the WebGPU renderer
  and WebGL fallback, glTF assets, Biome for lint and format, Vitest for unit
  tests, Playwright for browser tests. pnpm workspace.
- Input: keyboard via the DOM and gamepad via the Gamepad API, both mapped to
  one input struct before anything else sees them.
- Simulation and server: Rust. The same sim crate compiles to WebAssembly for
  the browser and runs natively on the server (ADR 0002, ADR 0005).
- Infrastructure: OpenTofu, applied by hand (ADR 0015).

## Consequences

- The TypeScript client knows nothing about physics; it renders what the sim
  reports.
- Every Rust crate must compile for `wasm32-unknown-unknown` or be clearly
  server-only.
- Three.js and WebGPU API changes are tracked by the web app alone.
