# Formula None

A browser kart racer for up to eight friends: one link, a four-letter room
code, no install, no account. Rust simulation shared between the browser and
the server; Three.js for the picture.

## Getting started

Requirements: Rust stable (via rustup; `rust-toolchain.toml` adds clippy,
rustfmt and the wasm target), `cargo-nextest`, Node 22 and pnpm 11.

```sh
cargo install cargo-nextest --locked
pnpm install
```

| Side | Build                    | Lint                                        | Test               |
| ---- | ------------------------ | ------------------------------------------- | ------------------ |
| Rust | `cargo build --workspace` | `cargo clippy --all-targets -- -D warnings` | `cargo nextest run` |
| Web  | `pnpm build`             | `pnpm lint`                                 | `pnpm test`        |

`pnpm test:e2e` runs the Playwright smoke test against the Vite dev server
(run `pnpm --filter web exec playwright install chromium` once).

## Reading order

1. `CLAUDE.md` for the layout, the dependency rule and the commands.
2. `docs/GLOSSARY.md` for the vocabulary.
3. `docs/adr/` for the decisions, numbered in the PRD's order.
