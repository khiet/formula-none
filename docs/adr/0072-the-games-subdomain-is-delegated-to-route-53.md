# ADR 0072: The game's subdomain is delegated to Route 53

## Status

Accepted (2026-11-01). Supersedes the DNS and profile lines of ADR 0015.

## Context

ADR 0015 put the A record in "the hosted zone that holds `sushimafia.com`" and
assumed that zone already existed in Route 53. It does not: the domain's DNS and
its mail live at the registrar, 123-Reg. The first apply, for issue 225, also
showed that the `neconote` profile ADR 0015 names is a limited user that cannot
create this stack.

## Decision

- Route 53 holds a public hosted zone for `formulanone.sushimafia.com` alone,
  created by hand in the same account. The A record sits at that zone's apex.
- Four NS records for `formulanone` at the registrar delegate to that zone. The
  rest of the domain, its mail included, stays where it is.
- The module applies with a profile named `formula-none-admin`, an administrator
  key used only for applies and removed between them. Nothing here runs from CI.

## Consequences

- The domain's mail is not at risk from the game's DNS, and moving the rest of
  the domain to Route 53 stays a separate decision.
- The zone is not in the module, so it survives a `tofu destroy` and its USD 0.50
  a month is the cost ADR 0015 already counts.
- Replacing the zone changes its nameservers, so the NS records at the registrar
  must be replaced with it.
- An apply needs a fresh key for that profile first, about two minutes of setup,
  in exchange for no standing administrator key on the laptop.
