# ADR 0020: Camera feel values in the tuning module

## Status

Accepted (2026-10-23). Widens the scope of ADR 0014.

## Context

ADR 0014 keeps the gameplay, item, bot and network constants in
`sim::tuning`. The chase camera's pull-back at top speed and its shake on a
hit are Presentation: the sim never reads them. They are still tuned by
feel, in the same pass as the handling they answer to, and issue 25 asks
for them to live in the tuning module.

## Decision

- Constants that Presentation reads to answer the sim's feel, starting with
  the camera's pull-back and shake, live in `sim::tuning` under their own
  heading and reach the web app through the bridge like any other tuned
  value.
- The sim never reads them; a compile-time invariant may still relate them
  to gameplay values, as the shake is held shorter than a spin-out.
- Values that are only how a tell looks, such as a spark's size or a mark's
  colour, stay in the Presentation module that draws them.

## Consequences

- A feel pass over handling and camera is one diff in one file.
- The bridge grows one getter per camera value, as it does for every tuned
  value the client needs (ADR 0003).
