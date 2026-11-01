# ADR 0071: The budget counts the game's own cost

## Status

Accepted (2026-11-01). Supersedes no ADR: ADR 0017 sets the ceiling and says
nothing of how the alert measures it; the account-wide scope was only in the
configuration and `infra/README.md`.

## Context

Issue 225 made a billing alert a precondition of making the repository public.
The first version watched the whole account, on the view that the account held
nothing else. It does not: the environment shares an account with other
workloads, whose bill alone can pass USD 25. An account-wide budget would alert
every month and say nothing about the game. A separate account would have kept
the scope simple, at the cost of more setup than the game needs today.

## Decision

- The USD 25 budget counts only cost tagged `Project=formula-none`, the tag the
  provider's `default_tags` puts on every resource the module creates.
- `Project` is activated once by hand as a cost allocation tag, after the first
  apply, since AWS cannot activate a tag before it has seen it.

## Consequences

- The alert speaks about the game alone, and the ceiling of ADR 0017 is unchanged.
- Cost that carries no tag goes uncounted, so the budget reads slightly under
  the real cost: a hosted zone made outside the module, and anything AWS bills
  without a resource tag. The margin of ADR 0015's estimate under the ceiling
  covers this.
- The budget reads zero until the tag is activated, and it does not count cost
  from before then, so the first days of the environment go unwatched.
- Anything created in the account outside this module no longer trips the
  alert.
