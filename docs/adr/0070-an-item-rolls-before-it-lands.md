# ADR 0070: An item rolls before it lands

## Status

Accepted (2026-10-31). Supersedes no ADR: none states that a rolled item is
ready at once.

## Context

A box taken into an empty slot put the item there on the same tick, ready
to fire, and nothing on the road marked the moment. Issue 249 asks for a
pickup the player notices: the box breaks with a sound, the slot spins
through the items for a moment and lands on one, as a kart racer's item
roulette does. The picks for the turn, the sounds, the reel and the landing
are compared in `docs/mockups/item-box-roll.html`. ADR 0068 sets which
karts take a box; this record sets what the slot does once one does.

## Decision

- The item slot is empty, rolling or held. A box taken into an empty slot
  starts a roll of the tuned `ITEM_ROLL_S`, 2.5 s, toward the item the
  server drew at the take; when the roll runs out the item is held.
- A rolling slot is full. A box taken mid-roll gives nothing, as one taken
  holding an item does (ADR 0068), and the roll runs on.
- A press of the item button while the slot rolls fires nothing and is not
  remembered: the item fires on a press after it lands. A press on the
  landing tick is still a press during the roll.
- Nothing stops a roll early: it runs on through a spin-out, a respawn and
  the finish.
- `ItemPicked` still names the item at the take. The snapshot carries the
  slot, with the roll's time left, so a client knows when it lands.
- A bot's item delay runs from the landing, not the take. A bot keeps a
  boost while it waits for the wheel, and while it would have to wait at
  the boosted speed, so a boost it holds longer never throws it into a pass
  it judged at its own speed.

## Consequences

- The protocol's `KartSnapshot` carries `itemSlot` in place of `heldItem`,
  and the protocol version goes up. The sim's readout does the same.
- A client shows the item from the snapshot that says it landed; until a
  later change gives the slot its reel, it shows the slot empty while the
  item rolls.
- Every replay fixture whose karts take a box was re-recorded: items fire
  2.5 s later, so everything after the first take happens differently.
- Bots fire later after a take, so their races play out differently; the
  sim tests that pinned bot timing around items now race without boxes,
  and the wheel tests let off a bot that an item spun out on the crossing.
- The View's reel, the box's turn and the sounds are a separate change.
