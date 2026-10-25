# ADR 0008: Audio

## Status

Accepted (2026-10-18). Superseded by ADR 0025.

## Context

Every mechanic needs a tell; sound is half of it. The game is free with no
tracking, so audio preferences stay in the browser.

## Decision

- Web Audio. The engine note is a looping oscillator with pitch bound to
  speed. SFX (hop, mini-turbo release, item pickup, item fire, hit, lap) are
  preloaded. One CC0 music loop.
- Three toggles (SFX, engine, music) persisted in localStorage alongside the
  nickname.

## Consequences

- Audio is Presentation: it reads sim state and events and never changes
  them.
- No audio asset pipeline beyond static files; no audio middleware.
