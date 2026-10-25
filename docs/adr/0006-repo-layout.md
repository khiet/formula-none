# ADR 0006: Repo layout

## Status

Accepted (2026-10-18). The one-way rule, where it keeps protocol off sim,
is superseded by ADR 0026.

## Context

Four Rust crates, a web app, infrastructure, art sources and documentation
all ship together and change together. Agents and humans need to find the
dependency direction from the layout alone.

## Decision

One monorepo:

- `crates/` is a Cargo workspace with `sim`, `protocol`, `bridge` and
  `server`.
- `apps/web` is the Vite app in a pnpm workspace; `tools/` holds workspace
  packages that are not shipped, such as the repo-checks tests.
- `infra/` holds OpenTofu.
- `assets/` holds Blender sources and exported glTF.
- `docs/` holds the ADRs and the GLOSSARY. `CLAUDE.md` at the root states
  the rule below and the commands.

The one-way dependency rule: the web app depends on the bridge; the bridge
and the server depend on sim and protocol; sim depends on nothing in the
repo.

## Consequences

- A dependency in the wrong direction fails CI: the repo-checks tests read
  the Cargo manifests and the web app's package.json.
- One issue, one branch, one PR; every PR runs the full CI (ADR 0016).
- Rust and web tooling are versioned together, so a scaffold change is one
  commit.
