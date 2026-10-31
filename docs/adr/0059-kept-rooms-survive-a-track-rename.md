# ADR 0059: Kept rooms survive a track rename

## Status

Accepted (2026-10-31). Supersedes the rule in ADR 0053 that a room whose
track the new server does not know is dropped and logged. What ADR 0053
decides about which rooms are kept, the volume, the files and the grace
stands.

## Context

ADR 0053 keeps every Open room on a volume across a deploy and drops a
kept room that names a track the new server lacks, on the reasoning that a
rename between deploys costs that room and nothing else. Issue 211 renames
Suzuqa Patch to Bell Deer and its id from `brands-patch` to `bell-deer`,
the first such rename. Every room on the volume at that deploy races the
default track, so under ADR 0053 the deploy would empty every open room:
the very evening-ending loss ADR 0053 was written to prevent.

The track a room races carries no choice of the players today; every room
races the default (rooms do not yet pick a track). Bringing the room back
on another track costs them nothing they chose.

## Decision

- The server keeps a short table of old track ids, each with its new id
  (`RENAMED` in `crates/server/src/tracks.rs`). A kept room naming an old
  id is restored on the new one, silently. `brands-patch` maps to
  `bell-deer`; an entry stays as long as a file written under the old id
  may still be on the volume.
- A kept room naming any other track the server lacks is restored on the
  default track and logged as `room_track_unknown`, with the name it had
  and the track it races instead. It is no longer dropped.

## Consequences

- A rename between deploys loses no room. A rename adds a line to the
  table in the same change.
- A track removed outright moves its rooms to the default rather than
  closing them. Once rooms pick a track, that is a change the players did
  not ask for; the log line makes it visible, and the decision is revisited
  when track choice lands.
- Tested through the fake transport (`crates/server/tests/restart.rs`): a
  kept room on `brands-patch` rejoins and its race starts on `bell-deer`
  with no warning; a kept room on an unknown track races Bell Deer with
  the warning logged.
