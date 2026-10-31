# Contributing

Issues are welcome: bugs, ideas, a race that felt wrong. Say what you did,
what you expected and what happened; a room code or a screenshot helps.

Pull requests are welcome for issues labelled `help wanted` only, and the
ones also labelled `good first issue` are the place to start. Comment on the
issue before you begin so two people do not build the same thing. A pull
request for anything else will be closed with thanks, since the design is
settled in the ADRs first.

- One issue, one branch, one pull request, with Conventional Commits that
  reference the issue.
- The conventions live in [`CLAUDE.md`](CLAUDE.md): the layout, the one-way
  dependency rule, the vocabulary in `docs/GLOSSARY.md` and the commands.
- `pnpm check` runs what CI runs, in the same order; it is the gate, and a
  pull request that fails it is not reviewed.
- A change to what the game draws updates `docs/style-guide.html`; a change
  to a decision gets a new ADR under `docs/adr` rather than an edit.
