# ADR 0016: CI and deploy

## Status

Accepted (2026-10-18)

## Context

Shipping must be routine and a PR that touches nothing must go green, so the
pipeline is defined before any feature exists.

## Decision

On every PR, GitHub Actions runs:

- `cargo nextest run` and `cargo clippy --all-targets -- -D warnings`
- the wasm build of the bridge and the native-vs-wasm replay hash test
- the ts-rs generation diff check
- Biome, Vitest, and the Playwright lobby smoke test

On merge to main: build a multi-arch image for arm64, push it to ECR tagged
with the commit, then run SSM Run Command on the instance to `compose pull`
and `compose up`. The same deploy is one command runnable by hand. A smoke
script hits the health endpoint and performs one WebTransport handshake
after each deploy and fails the job if either fails.

The scaffold wires clippy, nextest, the wasm build, Biome, Vitest and the
Playwright smoke test; the replay hash, the ts-rs diff check and the deploy
join the workflow with the features they check.

## Consequences

- Clippy warnings are errors everywhere, including tests.
- A deploy mid-race ends the race; acceptable at friends scale.
- CI's only AWS permissions are ECR push and one SSM command (ADR 0015).
