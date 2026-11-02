# ADR 0074: Pull requests build the image

## Status

Accepted (2026-11-02). Supersedes no ADR: it adds a check to the pull request
list in ADR 0016.

## Context

ADR 0016 builds the image only on a merge to main, so a broken image first
shows as a failed deploy, and the site stays undeployed until a fix lands. The
first deploy found four such problems, all in the `Dockerfile` or
`.dockerignore`, and no pull request check could have caught them.

## Decision

- An `image` job in the CI workflow builds the root `Dockerfile` on every pull
  request, with the deploy job's build inputs and `push: false`, sharing its
  `type=gha` cache. Repo-checks holds the two builds to the same inputs.
- The job has no AWS role, no secrets and a read-only token, so a fork's pull
  request runs it the same way.
- It does not run on main, where the deploy job builds the same image, and the
  deploy job does not need it: a job whose need was skipped is skipped too.
- There is no paths filter. Telling which changes can reach the image means
  keeping a second list beside `.dockerignore`, and that list going stale is
  the failure this check exists to catch.
- `pnpm check` does not run it, since that would make Docker a requirement of
  the local gate.

## Consequences

- Every pull request pays for one more build; the image step took about 7
  minutes in the deploy that first succeeded.
- `pnpm check` can pass on a change that breaks the image, so this job is the
  first to show it unless the build is run by hand with the command in
  `deploy/README.md`.
- Whether a failure blocks the merge is a branch protection setting on main, not
  something this workflow decides.
