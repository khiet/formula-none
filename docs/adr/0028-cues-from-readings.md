# ADR 0028: Audio cues stay edges between readings

## Status

Accepted (2026-10-25). Rejects the event-driven alternative to ADR 0025's
rule that cues are edges between two readings of the own kart.

## Context

The architecture review of 2026-10-25 (issue 90, candidate 4) proposed
that the bridge expose the sim's `RaceEvent`s, the race readout carry
events from both runs, and `soundCues` map events to cues instead of
diffing two readings.

Against the code after issues 87 to 89, the review found:

- The sim names five of the thirteen cues: item pickup, item fire, hit (a
  spin-out), lap and finish. The other eight have no event: countdown
  beep, go, hop, mini-turbo (which also plays on a rocket start's
  release), boost pad, tier chime, sputter and wrong way. `soundCues` would
  keep diffing readings for them.
- The two edges the review called guesses, the boost pad and a drift's
  release against its end, are among those with no event.
- The five with an event already reach the readings. In a room the item
  slot changes only on the server's `itemPicked` and `itemFired`, a hit
  arrives as the spin-out timer in a snapshot, and laps and finishes come
  from the snapshots and the `finished` event. The edge `soundCues` sees is
  the event, one frame later, with no second channel.
- The item fire flash timers in both runs drive the item slot, not the
  audio, so they stay either way.
- The change deletes about eight lines of `soundCues` and adds a bridge
  accessor, its TypeScript wrapper, an events field on the race readout and
  draining in both runs: roughly fifty.

## Decision

- Cues stay edges between two readings of the own kart, as ADR 0025 says.
  The bridge does not expose the sim's events to the client for audio.
- A new cue whose edge the readings cannot show gets a field on the kart
  readout (as `boost_s` was added), not an event channel.

## Consequences

- `soundCues` stays the one place that knows every cue; its tests drive it
  with two readings.
- A cue whose edge no readout field can carry is the case to revisit this
  for.
