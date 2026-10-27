# Formula None

Browser kart racer for up to eight friends by room code. One Rust sim shared
between the browser (WebAssembly) and the server; Three.js renders what the
sim reports. The PRD is issue 1; each decision in it is an ADR in `docs/adr`.

## Vocabulary

Use the words in `docs/GLOSSARY.md` exactly: kart, room, creator, tick,
snapshot, checkpoint, spline progress, hop, drift, mini-turbo, spin-out,
livery, reconciliation, Scene, View, Instrument panel, plate. When code and glossary
disagree, fix the code.

## Layout and the one-way dependency rule

```
crates/sim        rules of the game; deterministic step(state, inputs, dt)
crates/protocol   wire messages; TypeScript types are generated from here
crates/bridge     wasm-bindgen surface over sim and protocol
crates/server     authoritative server binary
apps/web          Vite + Three.js client (TypeScript strict)
tools/            workspace packages that are not shipped (repo-checks)
infra/            OpenTofu for the single-instance AWS environment
deploy/           Compose stack, Caddyfile, and the deploy and smoke scripts;
                  the Dockerfile at the root packages them with the server
assets/           track layouts, the Blender export and the exported glTF plus sidecars;
                  the kart's build script and its export; the Kenney pixel font the
                  Instrument panel sets its type in; replay fixtures for the
                  determinism gate; LICENSES.md for what came from where
docs/             GLOSSARY.md, adr/, and mockups/: the design comparisons the
                  ADRs cite, plus asset-catalog.html, every graphic drawn in
                  the base design with the drift still to fix
```

Dependencies flow one way: web depends on the bridge; the bridge and the
server depend on sim and protocol; protocol depends on sim for the
conversions between their types (ADR 0026); sim depends on nothing in the
repo. The repo-checks tests read the Cargo manifests and the web app's
package.json and fail CI when a dependency points the wrong way. `crates/sim/src/tuning.rs` is the single home for gameplay, item,
bot and network constants (ADR 0014).

## Commands

Rust (from the repo root):

- Build: `cargo build --workspace`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test: `cargo nextest run`
- Re-record the replay fixtures after an intended rules change:
  `cargo run -p sim --example replay -- --update assets/replays/*.replay.json`
  (see `assets/replays/README.md`)

Web (from the repo root, after `pnpm install`):

- Bridge to wasm: `pnpm build:bridge` (needs `wasm-pack`; rerun after
  changing sim or bridge; output is gitignored and the other web commands
  need it)
- Regenerate the protocol's TypeScript types and fixtures, and the sim's
  kart readout and tuning types, after changing `crates/protocol` or those
  sim types: `pnpm generate:protocol` (output is committed under
  `apps/web/src/generated/protocol` and `apps/web/src/generated/sim`; CI
  fails on a stale copy)
- Build: `pnpm build`
- Lint: `pnpm lint` (`pnpm lint:fix` applies Biome's fixes)
- Typecheck: `pnpm -r typecheck`
- Test: `pnpm test` (`pnpm test:e2e` runs Playwright against the dev server with
  the browser muted; `E2E_SOUND=1 pnpm test:e2e` to hear it)

Tracks (needs Blender on `PATH`; see `assets/tracks/README.md`):

- Export: `pnpm export:tracks` writes each track's glTF and sidecar under
  `assets/tracks/<name>/`; rerun after changing a layout and commit the
  output, since the sim's tests read the sidecars
- Check the layouts without Blender: `pnpm test:tracks` (needs `python3`)
- Kart: `pnpm export:karts` builds `assets/karts/kart.glb` from
  primitives; rerun after changing `assets/karts/build.py`

Audio (needs `python3` on `PATH`):

- `pnpm export:audio` synthesises the SFX under `assets/audio/` and leaves
  the hand-placed music loop alone; rerun after changing
  `assets/audio/build.py` and commit the output

CI (`.github/workflows/ci.yml`) runs on every PR: clippy with warnings as
errors, nextest, the wasm build of the bridge, the protocol codegen diff,
Biome, typecheck, Vitest, the web build, the Playwright smoke test, and the
replay determinism job that reruns the fixtures under `assets/replays`
natively and in wasm.

`pnpm check` runs the same checks locally in that order (about 6 minutes) and
is the suite to run before opening a PR. Its e2e step sets `CI=1`, so it starts
fresh servers and fails fast if a stale one already holds port 8080 or 5173.

## Working here

- One issue, one branch, one PR. Commit messages follow Conventional Commits
  and reference the issue.
- Tests drive the system through a public seam (the sim's `step`, the
  protocol's encode and decode, the fake transport, the Playwright lobby
  flow) and assert observable outcomes. No tests of render output.
- A decision that changes an ADR gets a new ADR that supersedes the old one;
  ADRs are not edited to say something different.
