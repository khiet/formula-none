# ADR 0028: Audio cues stay edges between readings

## Status

Accepted (2026-10-25). Restates ADR 0025's "cues are edges between two
readings of the own kart" with the reason it holds; changes nothing.

## Context

The architecture review of 2026-10-25 (issue 90, candidate 4) proposed
that the bridge expose the sim's `RaceEvent`s, the race readout carry
events from both runs, and `soundCues` map events to cues instead of
diffing two readings.

Against the code after issues 87 to 89, the review found:

- The sim names five of the thirteen cued edges: item pickup, item fire,
  hit (a spin-out), lap and finish. The countdown beep, go, hop, mini-turbo
  and rocket release, boost pad, tier chime, sputter and wrong way have no
  event, so `soundCues` would keep diffing readings for them.
- The two edges the review called guesses, the boost pad and a drift's
  release against its end, are among those with no event.
- In a room the server's events arrive with the snapshot and are never
  predicted. An event-driven pickup or fire cue for the own kart would sound
  a round trip after the predicted kart reached the box or let go of the
  item; the reading the prediction reports has the edge on the tick it
  happens.
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

- Audio in a room keeps the prediction's timing for the own kart.
- `soundCues` stays the one place that knows every cue; its tests drive it
  with two readings.
- A review that finds the edges re-derived should weigh the unpredicted
  server events and the eight cues the sim does not name before proposing
  events again.
