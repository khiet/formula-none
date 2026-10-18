# Glossary

The vocabulary of Formula None. Code, tests, issues and ADRs use these words
with these meanings; when a word here and a word in the code disagree, fix the
code. Terms come from the PRD (issue 1); the client terms reuse the Purrjump
glossary where they fit.

## Race

### kart

One racer's vehicle, driven by a human or a bot. All karts are identical at
v1; the livery and the nickname floating over it tell one from another.

### room

A lobby identified by a four-letter code from an unambiguous alphabet. Holds
up to eight players, has a creator, and moves through Open, Countdown, Racing,
Results and Closing. A room enters Closing five minutes after the last human
leaves.

### creator

The player who can press Start in a room. The role passes to the next player
if the creator leaves, so a room never gets stuck.

### tick

One fixed step of the sim, 60 per second on both client and server. Every
rule in the game is expressed per tick, never per frame or per wall-clock
second.

### snapshot

The server's authoritative state of every kart at a tick, sent at 30 Hz
together with the last acknowledged input sequence per client and the events
since the previous snapshot.

### checkpoint

An ordered gate on the track used for lap counting and wrong-way detection.
Brands Patch has twelve. A kart respawns at its last checkpoint facing the
next one.

### spline progress

Distance along the track centreline. Race position is ordered by lap, then
checkpoint index, then spline progress. Decreasing spline progress for two
seconds triggers the wrong-way warning.

### hop

A short jump (0.3 s airtime) that starts a drift when it lands while the kart
is steering above 40 percent of top speed.

### drift

Sliding through a corner after a hop. The drift direction is fixed at the hop;
counter-steer widens the arc and inward steer tightens it. Holding a drift
charges a mini-turbo.

### mini-turbo

The boost released at the end of a long enough drift: blue at 0.8 s of drift
(0.6 s boost), orange at 1.6 s (1.0 s boost), both at +30 percent top speed.
Shown by sparks at the wheels while charging.

### spin-out

A kart stopped and spun by an item for a fixed time (1 s). Every spin-out is a
server event; clients only show it.

### livery

The fixed colour that tells one kart from another. Eight liveries, assigned in
room join order.

### reconciliation

The client replacing its predicted state with a snapshot and replaying the
inputs the server has not yet acknowledged. The visible correction is blended
over up to six frames rather than snapped.

## Track

### sidecar

The data file exported from Blender next to a track's glTF: the centreline,
checkpoints, item box and boost pad positions, the spawn grid and the tagged
collision meshes. The sim reads the sidecar and never the glTF.

### surface tag

What is under a point on the track: asphalt, sand, grass, wall, or out of
world when nothing is below it. Collision meshes in the sidecar carry the
first four; the sim answers with the topmost one under a point.

## Client

### Scene

One screen of the client with its own lifecycle: Title, Room, Race, Results.
Exactly one Scene is active at a time.

### View

The part of a Scene that renders the world. The Race Scene has one View, the
3D track, drawn by Three.js from kart transforms the bridge reports.

### Instrument panel

The HUD overlaid on a View: position, lap, lap time, best lap, item slot,
wrong-way warning and connection banner. Reads state; never drives it.

### Presentation

Everything the player sees and hears that is not a rule: meshes, materials,
particles, camera motion, audio. Presentation never changes sim state.

### Feedback

A piece of Presentation tied to one mechanic so the mechanic has a visible or
audible tell: tyre marks on drift, sparks on mini-turbo charge, a dust puff
off-track, a spin animation on spin-out.
