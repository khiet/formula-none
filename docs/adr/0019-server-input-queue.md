# ADR 0019: Server input queue

## Status

Accepted (2026-10-19). Supersedes the input handling recorded in the
Outcomes of ADR 0005 and ADR 0009.

## Context

Issue 17 had the server apply the newest input each human had sent and
hold it until a newer one arrived. Whenever a datagram landed either side
of a tick, that skipped one input and applied another twice. The client's
prediction (ADR 0009) replays its inputs one per tick, so it cannot follow
a skip it cannot see: at 100 ms RTT on the fake transport, mid-drift
corrections reached 0.8 m.

## Decision

- The server applies one input sequence per tick, in order, so the tick an
  input is applied at is fixed by its sequence.
- It starts applying a human's inputs once it holds
  `INPUT_QUEUE_START_TICKS` (two), so the queue starts one input ahead.
- A sequence that has not arrived by its tick is applied as the last
  controls, and is dropped if it arrives later.
- The applied sequence stays within `INPUT_QUEUE_MAX_TICKS` (three) of the
  newest received. Further ahead of the client, the server waits for it,
  so a client back from a stall is heard again. Further behind, it drops
  the oldest inputs, so a client whose clock runs fast adds no delay.
- A snapshot acknowledges the sequence applied at its tick.

## Consequences

- Once a snapshot of tick T acknowledges sequence A, sequence A + n is
  applied at tick T + n. The client lays its predicted inputs out on the
  server's ticks by that lead, which moves only when a client stalls or the
  server drops inputs to catch up.
- A lost input costs one tick of the last controls. Mid-drift, with a
  bang-bang steer, that makes a correction of up to about 0.2 m.
- The start costs one tick of input delay.
- A snapshot can acknowledge a sequence the server never received.
