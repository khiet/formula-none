# Architecture Decision Records

One file per decision, numbered in the order the PRD (issue 1) lists them.
A record is never edited to say something different; a new record supersedes
it and the old one's status changes to `Superseded by ADR NNNN`.

Format: `NNNN-short-slug.md` with the sections Status, Context, Decision,
Consequences. The repo-checks package fails the build if a record is missing
or malformed.
