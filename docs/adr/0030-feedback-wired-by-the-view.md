# ADR 0030: The View wires each Feedback by hand

## Status

Accepted (2026-10-25). Rejects a shared interface over the Feedback the
View draws.

## Context

The architecture review of 2026-10-25 (issue 90, candidate 6) proposed one
Feedback interface over the sparks, dust, tyre marks and kart shadows,
each taking the race readout, with the View iterating a list. The spin on
a spin-out turns the kart's own mesh and was not part of the proposal.

Against the code after issue 89 handed the View one race readout, the
review found:

- Each Feedback already decides from a kart readout and its store is tested
  through it (`sparks`, `dust`, `tyre-marks`, `kart-shadows`). The
  interface would add no test the stores lack.
- The one invariant the View holds, that stores fill before their meshes
  redraw, is visible only in render output, which tests do not reach.
- The interface and four adapters add about ten to thirty lines more than
  they remove from `race-view.ts`.

## Decision

- The View constructs each Feedback and calls it in its frame, store
  updates before mesh updates, as it does now. No shared Feedback
  interface.

## Consequences

- A new Feedback touches `race-view.ts` in four or five places: import,
  construction, adding its mesh, its per-frame call and its mesh update.
- The store-before-mesh order stays the order of the calls in the View's
  frame, untested.
- Revisit when the Feedback count makes the View's frame hard to read, or
  when a Feedback needs a lifecycle (a dispose) the others share.
