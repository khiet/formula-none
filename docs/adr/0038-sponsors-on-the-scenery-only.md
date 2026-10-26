# ADR 0038: Invented sponsors stay on the scenery

## Status

Accepted (2026-10-26). Supersedes the sponsor clause of ADR 0035's "No team
name, crest, sponsor or race number appears in the game": the ban on
sponsors now covers the karts and real sponsors only. Team names, crests
and race numbers stay banned everywhere.

## Context

ADR 0034 and ADR 0035 both state the ban inside their livery decisions: a
kart borrows a team's colours, and nothing else of the team. ADR 0035 words
it as "appears in the game", which, read literally, also bans the twelve
joke sponsors on Brands Patch's hoardings (`assets/tracks/trackgen/sponsors.py`),
Chicane Cola, Late Braking Bank and the rest. Every one is invented here
and none points at a real company.

The fiction is a cartoon parody of Formula 1, and trackside boards are part
of what a circuit looks like. The joke names are the parody's, in flat ink
on white boards, which already fit the flat low-poly look (the asset
catalog's Sponsor hoardings card). Removing them would leave the start
straight and the final corner bare for no gain against the risk the ban
guards: a kart that reads as a real team's, carrying a real sponsor.

## Decision

- No kart carries a sponsor, real or invented: a livery is colours only.
- The scenery may carry invented sponsors. Each is a joke name of our own,
  in our own lettering, and resembles no real company, brand or logo.
- No real sponsor appears anywhere in the game.
- No team name, crest or race number appears anywhere in the game, as
  ADR 0035 decides.

## Consequences

- Brands Patch keeps its hoardings, and the tracks need no re-export.
- `trackgen/sponsors.py` is the one home of the scenery's sponsors; a new
  board's name goes there and is invented.
- The asset catalog's Sponsor hoardings card is aligned, not open.
