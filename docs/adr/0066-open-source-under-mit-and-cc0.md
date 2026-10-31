# ADR 0066: Open source under MIT for the code and CC0 for the art

## Status

Accepted (2026-10-31).

## Context

ADR 0007 chose CC0 art "so the repo can become public under MIT with CC0
assets". The repository is documented like a showcase already: the ADRs, the
glossary, the style guide and the shared sim with its determinism gate.
Issue 225 makes it public. An audit of the tree and every commit found no
secrets and only permissive dependencies, and one licensing hole: the music
loop had been swapped in PR 59 for a track with no recorded source, while
`assets/LICENSES.md` claimed every WAV was synthesised.

## Decision

- The existing repository is made public in place, history, issues and PR
  discussions included, since the ADRs cite issue numbers.
- The code is MIT, as the root `LICENSE` says, and every package manifest
  declares it. The art is CC0 1.0: everything under `assets/`, the item
  sprites and marshal drawn in the web app's source, and the images the repo
  makes under `docs/` and `apps/web/public`. `assets/LICENSES.md` records
  each, and `tools/repo-checks` holds it to the tree.
- Every sound is synthesised by `assets/audio/build.py`. The music loop is
  the 8 s loop the game first shipped with; a file whose source the repo
  cannot show does not ship.
- Contributions: issues are welcome; pull requests only for issues labelled
  `help wanted`. No CLA, no DCO, no code of conduct beyond GitHub's default.
- The hosted instance stays up. `infra/` describes the author's own
  deployment; self-hosters use the Compose stack under `deploy/`.

## Consequences

- The music is 8 s again until a longer synthesised loop replaces it
  (issue 227).
- A new asset needs a row in `assets/LICENSES.md` that names its source; a
  sound needs a `write` in `assets/audio/build.py`.
- A secret found in history later is rotated, not rewritten out of history.
