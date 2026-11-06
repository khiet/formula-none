# ADR 0094: The client reads the sim's tuning by its generated name

## Status

Accepted (2026-11-06). Supersedes ADR 0020's consequence that the bridge
grows one getter per camera value. ADR 0014, ADR 0026's one `tuning()` call
and the rest of ADR 0020 stand.

## Context

ADR 0026 hands the browser the sim's tuning in one call, decoded into the
generated `Tuning`. The bridge then copied 36 of its fields onto the
`Bridge`, 29 under a new name (`stopCleanS` as `cleanStopSeconds`,
`cameraShakeS` as `shakeSeconds`), and each consumer declared its own shape
of the renamed fields, which the Race Scene built by hand. A tuned value the
client read was named three times: in `tuning.rs`, in the bridge's rename
and in each consumer's shape. The kart readout's generated names
(`spinOutS`, `boostS`, `starS`) were already read as they are.

## Decision

- The `Bridge` exposes `tuning: Tuning`, the generated type, once. It copies
  and renames none of its fields.
- Each consumer takes a `Pick<Tuning, ...>` of the fields it reads: the
  View, the Instrument panel, the speed cluster, the chase camera, the pit
  boxes, the audio and its cues, and the BOX call's cliff. The Race Scene
  passes `bridge.tuning` to each.
- A value the client derives from the tuning has one named derivation beside
  its reader: the WebTransport timeout in `connection.ts`, the ping timing
  in `ping.ts`, the reconnect timing in `reconnect.ts` and the nickname
  bounds in `nickname.ts`, each in milliseconds or characters from the
  sim's seconds and counts; the kart's box is destructured from `kartSizeM`
  where it is read.
- What the client chooses rather than the sim (whether a course has a pit
  lane, the laps picked, how long the BOX call shows) stays out of `Tuning`.

## Consequences

- A tuned value the View reads is a constant in `tuning.rs`, the
  regenerated `Tuning.ts`, a name in the View's `Pick` and the line that
  reads it. The bridge and the Race Scene do not change.
- Client code and test fixtures spell tuned values as `tuning.rs` does, with
  their unit suffixes (`S`, `M`, `MPerS`, `Percent`).
- No tuned value and no behaviour changes, so no replay fixture is
  re-recorded.
