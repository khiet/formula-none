# Formula None

A browser kart racer for up to eight friends: one link, a four-letter room
code, no install, no account. Rust simulation shared between the browser and
the server; Three.js for the picture.

## Getting started

Requirements: Rust stable (via rustup; `rust-toolchain.toml` adds clippy,
rustfmt and the wasm target), `cargo-nextest`, `wasm-pack`, Node 22 and
pnpm 11.

```sh
cargo install cargo-nextest wasm-pack --locked
pnpm install
pnpm build:bridge
```

`pnpm build:bridge` compiles the bridge crate to WebAssembly into
`apps/web/src/generated/bridge` (ignored by git). The web app, its typecheck
and its tests need it; rerun it after changing `crates/sim` or
`crates/bridge`.

| Side | Build                    | Lint                                        | Test               |
| ---- | ------------------------ | ------------------------------------------- | ------------------ |
| Rust | `cargo build --workspace` | `cargo clippy --all-targets -- -D warnings` | `cargo nextest run` |
| Web  | `pnpm build`             | `pnpm lint`                                 | `pnpm test`        |

`pnpm test:e2e` runs the Playwright smoke test against the Vite dev server
(run `pnpm --filter web exec playwright install chromium` once).

## Controls

Keyboard only. Arrows and WASD do the same thing.

| Action   | Keys                 | Notes                                                                            |
| -------- | -------------------- | -------------------------------------------------------------------------------- |
| Steer    | Left/Right, A/D      |                                                                                  |
| Throttle | Up, W                |                                                                                  |
| Brake    | Down, S              | Reverses from a standstill.                                                      |
| Hop      | Space                | Land while steering to start a drift; keep it held, let go for the mini-turbo.   |
| Item     | Shift                | Fires the held item. Keep it held to trail a hazard behind you until released.   |

Rocket start: hold the throttle from the first half of the 2 through GO for
the full boost, from the second half for a weaker one; hold it from before
the 2 and the kart stalls. The beeps count 3, 2, 1 and GO.

## Reading order

1. `CLAUDE.md` for the layout, the dependency rule and the commands.
2. `docs/GLOSSARY.md` for the vocabulary.
3. `docs/adr/` for the decisions, numbered in the PRD's order.
