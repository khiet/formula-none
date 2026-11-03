# Glossary

The vocabulary of Formula None. Code, tests, issues and ADRs use these words
with these meanings; when a word here and a word in the code disagree, fix the
code. Terms come from the PRD (issue 1).

## Race

### kart

One racer's vehicle, driven by a human or a bot. All karts are identical at
v1; the livery and the nickname floating over it tell one from another.

### bot

The driver inside the sim for a kart no human drives: the slots humans leave
empty at Start, a human's kart once they disconnect or go silent, a
finished kart until the race ends, and every kart through the outro. A bot
reads only what any observer of the race can, the track and the karts'
transforms and standings, and answers with the same input struct a human's
controller produces. Bots start on the grid behind the
humans. How hard they drive is the room's bot level.

### bot level

How hard a race's bots drive: Basic, Pro or Ace, the creator's pick on the
Title Scene, fixed for the room as the course is and shown on the Room
Scene. Basic is the bots as they were before there was a pick. The level
varies a bot's corner speed, its rubber band's lower bound, its item use
delay and its rocket start chance, nothing else; it changes how the bots
drive, not how many there are. A replay carries it.

### race distance

How long a race runs, the creator's pick: Sprint for three laps or Grand
Prix for five, fixed for the room (ADR 0083).

### pit lane

The lane beside a course's start straight, from entry line to exit line,
where a kart goes to get new tyres. Safe ground: no item touches a kart in
it (ADR 0081). Not every course has one.

### pit limiter

What holds a kart to the pit lane's speed limit from entry line to exit
line, by itself (ADR 0081). Not a control.

### pit box

A slot's own place in the pit lane, where its pit crew waits and its kart
stops. Avoid: box, which is the item box; BOX is only the radio call on the
Instrument panel.

### pit crew

The six figures at a pit box who change a kart's tyres. Presentation only:
how long they take is the stop's roll.

### stop

A kart's time stationary in its pit box, rolled from the race seed, the slot
and the kart's stop count; it ends on green with tyre life back at 100% (ADR
0082).

### phase

Where a race is: Countdown, Racing, Outro or Finished. Sim state, stepped
like every other rule, so a solo race in the browser and a room on the
server run the same lifecycle. Countdown holds the karts on the grid for
the 5 s of the start lights, during which the throttle sets up the rocket
start.
Racing runs the laps and ends when every human's kart has finished or is
driven by a bot, or 60 s after the first human finished driving their own
kart; the unfinished are placed by progress at that point and the
standings become the results. Outro holds the Race View for 3 s with a bot
driving every kart and the FINISH banner up for every player.
Finished leaves the race as it is. A room's Countdown and Racing states
follow the race's phase, its Racing state covering the Outro too, and its
Results state its Finished (ADR 0032).

### start lights

The five lamps the Instrument panel lights one per second through the
countdown and puts out at Go, when GO shows for a second (ADR 0040).

### finished

A kart past the line on its last lap. Its finish time and place are final,
it counts no more laps, and a bot drives it until the race ends, taking
and firing no items; its human's inputs are ignored. A finished player
sees the FINISH banner with their place and the finishers so far in place
of the lap readout.

### rocket start

What the throttle hold ending at Go earns, read from the hold's length; a
release before Go resets it, and the throttle counts as held above half.
Its outcomes are the start boost and the stall. A hold longer than 2.0 s,
begun before the fourth light came on, stalls the kart: 2.0 s dead to every
control, as in a spin-out. A hold of 1.5 to 2.0 s, begun in the first half
of the fourth light, earns the full start boost, 1.5 s at +40 percent, the
boost item's values. A hold of 1.0 to 1.5 s, begun in the second half of
the fourth light, earns the weak start boost, 0.6 s at +30 percent, the
orange mini-turbo's values. A shorter hold, begun at the fifth light or
later, starts clean. Bots roll the full boost or a
clean start once per race from the seed and never stall; a kart handed to
a bot during the countdown starts clean. Each start light is beeped so the
start can be timed by ear; sparks show every charging kart's tier, orange for
the weak boost and red for the full one, with no warning once the hold
has tipped into a stall. The snapshots carry each kart's start and stall
time left, and a client's prediction adopts the server's verdict.

### rubber band

The scale on a bot's top speed by its spline distance from the nearest
human: slower far ahead, faster far behind, between the tuned bounds. The
lower bound varies by bot level, up to one at Ace, where a bot ahead is
never slowed; the upper bound is the same at every level. A human's kart
is never banded.

### room

A lobby identified by a four-letter code from an unambiguous alphabet. Holds
up to eight players, has a creator, and moves through Open, Countdown, Racing,
Results and Closing. A room enters Closing five minutes after the last human
leaves.

### session token

The secret a room deals a player at their join, which takes their seat back
over a new connection. Mid-race a bot drives the kart of a Disconnected
player until the token rejoins or the race ends; the seat, and the token
with it, is freed at the results. A seat an Open room brings back after a
server restart waits Disconnected for its token too, and frees after the
tuned grace if it never comes.

### creator

The player who can press Start in a room. The role passes to the next player
if the creator leaves, so a room never gets stuck.

### tick

One fixed step of the sim, 60 per second on both client and server. Every
rule in the game is expressed per tick, never per frame or per wall-clock
second.

### snapshot

The server's authoritative state of every kart at a tick, sent at 30 Hz
together with the input sequence applied at that tick per client and the
events since the previous snapshot.

### checkpoint

An ordered gate on the track used for lap counting and wrong-way detection.
Spaced evenly round the lap, twelve on the oval and fifteen on Bell Deer,
Goldstone and Monarch,
each reaching the
walls' outer faces, so a kart between the walls or in the air over one crosses
every one in order (ADR 0079).
A kart that leaves the world, or ends up behind a wall, respawns at its
last checkpoint facing the next one (ADR 0063); a kart that misses a checkpoint is not respawned and drives back
through it (ADR 0022).

### spline progress

Distance along the track centreline to the nearest centreline point within
reach of the last reading, so a kart keeps its own leg where the course
crosses over itself. Race position is ordered by lap, then
checkpoint index, then spline progress. Decreasing spline progress for two
seconds triggers the wrong-way warning.

### kart readout

What the sim says of one kart at a tick: its transform, speed and
velocity, drift and mini-turbo tier, item slot, the coins it carries,
spin-out, hop, boost, slipstream, rocket start and stall, and its place in
the race.
The sim reports one per kart every tick; a snapshot carries all of it but
the hop, which only the sim stepping the kart knows; a prediction's kart
adopts the snapshot's but for the boost, which it keeps its own. The View, the Instrument panel and
the bots read it. Not a snapshot, which is the server's message about
every kart.

### standing

Where a kart is in the race: its race position, laps completed, last
checkpoint crossed in order, spline progress, lap time, best lap, finish
time once it has finished and whether the wrong-way warning is up. The sim
reports one per kart every tick; the Instrument panel shows the local
kart's, and the Results Scene lists every kart's once the race is Finished.
Finishers hold their places in finishing order whatever they do after the
line.

### handling

How one kart answers its controls each tick: its speed, the hop, drift and
mini-turbo, boosts, the off-track cap, the freeze of a stall or spin-out,
and the hold of a wall and the bump off another kart. One rule in the sim,
run for every kart before the physics solver and told afterward of the
walls and karts it met.

### hop

A short jump (0.3 s airtime) that starts a drift when it lands while the kart
is steering above 40 percent of top speed. It lands once the airtime has run
out and the kart is back on the road, later where the road falls away under
it; a press in the air does not hop again.

### drift

Sliding through a corner after a hop. The drift direction is fixed at the hop;
counter-steer widens the arc and inward steer tightens it, as does a brake
drift. Holding a drift charges a mini-turbo.

### brake drift

Braking during a drift: the brake tightens the drift's turn on top of the steer,
in proportion to how hard it is held, and the mini-turbo keeps charging. It
still slows the kart, so held too long it takes the drift under 40 percent of
top speed, which ends the drift and loses its charge.

### mini-turbo

The boost released at the end of a long enough drift: orange at 0.3 s of drift
(0.6 s boost), red at 0.6 s (1.0 s boost), both at +30 percent top speed.
Shown by sparks at the wheels while charging, orange then red, and on
release by a flame at the exhaust in the tier's colour.

### slipstream

The pull of the kart ahead: a kart that holds another's wake for 1 s is
towed. No item and no button; open to every kart and every bot, so a race
stays close without the item table doing all the work. The server alone
judges it; a client's prediction takes its kart's charge and tow from the
snapshots (ADR 0077). The own kart feels it as wind lines building through
the charge and a whoosh as the tow takes; other karts show nothing.

### wake

The 12 m straight behind a moving kart, 1.2 m either side of its travel,
that the slipstream is judged in: a kart whose centre lies there, travelling
within 0.5 rad of the leader's travel, is in the wake, airborne or not. A
kart under half the tuned top speed, such as one spun out or stopped,
leaves none. Travel is the body's velocity, so a drifting leader's wake lies
along the road. Two wakes at once count as one.

### tow

What a full charge in a wake earns: the kart's top speed cap raised 15
percent, to 34.5 m/s with no coins, held at its full 1 s every tick the
kart stays in the wake and running down from the tick it leaves. A cap and
not a boost: the kart ramps up to it, the brake still works, a mini-turbo or
a pad under it fires and sets its own speed, and the kart then settles back
to the towed cap, not the plain one. The off-track cap still wins. A
spin-out, a respawn and a star's run clear it.

### item box

One of a track's fixed pickup spots, from the sidecar: eight on the oval
and twelve on Bell Deer, Goldstone and Monarch, in rows of four across the asphalt. A kart whose centre is on a box picks it up, whatever
it holds; the box is gone for three seconds and comes back. With an empty
item slot the kart rolls an item into it; with an item rolling or held, it
gets nothing, so a kart in front denies boxes to the karts behind (ADR 0068). Every
pickup is a server event naming the box, so every screen hides the same one.

### coin

A pickup lying on the asphalt, from the sidecar: twelve on the oval and
fifteen on Bell Deer and Monarch and thirteen on Goldstone, in lines along the
straights, ten of them on the
centreline. Also the count a kart carries, from none at the lights to a
full purse of ten. A kart whose centre is on a coin takes it unless it is
spun out; with a full purse its count holds, but the coin is still gone, so
a kart in front denies it to the karts behind. A taken coin is gone for
three seconds and comes back. Each coin carried raises the kart's top
speed by 1%, so a full purse cruises at 33 m/s, and every spin-out costs
three, or all the kart has; lost coins vanish, and the View sprays them out of the kart, where
nothing can pick them up. The server holds the count: a pickup and a loss
are server events, and the snapshots carry the count and the coins there
to pick up (ADR 0056, ADR 0067).

### tyre

A kart's wheel, the rubber the tyre marks and the tyre gauge are named
for. Not a coin, which is the pickup, nor a tyre wall, which is a
surface.

### item slot

A kart's one place for an item: a boost, a hazard, a homing projectile, a
star or an Air Horn.
It is empty, rolling or held. A box into an empty slot starts a roll that
lands the item 2.5 s later; until then a press fires nothing and is not
remembered, and nothing stops the roll early (ADR 0070). Rolling or held,
the slot is full: the kart still takes boxes but gets nothing from them. A
press of the item button fires a held item and empties the slot. Every fire
is a server event; the Instrument panel shows the local kart's slot, with
the fire predicted from the press until the server's event confirms it.
While the local kart's item rolls, the slot runs the reel: the items
scrolled up through it, slowing to a stop on the one that lands, a click on
each change. Other karts' rolls show nothing.

### roll

The item a pickup puts in the slot, drawn on the sim's RNG with weights by
the kart's race position band: the leaders mostly hazards, the back of the
field mostly homing, the star only from third place back and the Air Horn
mostly at the front and the middle. Drawn at the pickup, it rolls in the slot for 2.5 s
before it lands and can be fired (ADR 0070). Only the server rolls; a
client's prediction never picks up a box and holds what the snapshots say.

### hazard

The defending item. Fired, it drops 1 m behind the kart, on the surface;
with the item button held it trails there until the button is let go. It
spins out the first kart to reach it and is used up, never the kart that
dropped it until that kart has driven clear of it, and a dropped hazard is
gone after 30 s. Every snapshot carries the hazards on the track; a client
draws a trailed one behind the kart as it draws that kart.

### homing projectile

The attacking item. Fired, it chases the kart next ahead of its owner by
race position: along the spline at 1.6 times top speed, steering for a
point ahead on the centreline, and straight at the kart once within lock
range. Reaching the kart it is used up: on a hazard the kart trails, which
goes with it, else on the kart, which spins out. Fired by the leader, with
no kart ahead, it is wasted. Every snapshot carries the projectiles in
flight; a client draws them between snapshots as it draws the other karts.

### star

The comeback item. Fired, it gives the kart a run of 6 s: its top speed cap
raised a quarter, with the boost item's kick on the fire, immune to
hazards, projectiles and the runaway wheel, and spinning out every ordinary
kart it touches, which it passes through; two star karts bump as usual. A
mini-turbo or a pad during the run sets its own speed, then the kart settles
back to the star's cap; braking still works. A respawn keeps the timer and
the finish ends it; a second star puts it back to 6 s. Every snapshot
carries the seconds left, which every client shines the kart from through
the rainbow in the same phase, and plays the star music to (ADR 0075). In
the slot it is a faceted mirror ball on a chain, shining once landed.

### Air Horn

The item worth saving. Fired, it lets off one blast 7 m round the kart,
reaching only within 15 m of spline progress the shorter way round the lap,
so it never reaches through a crossover or across a barrier. Every other
kart in reach spins out, unless it trails a hazard, which is used up in its
place, or is a star kart or already spun out, which it leaves alone; every
hazard lying on the road in reach is used up, the firer's own included,
though not one the firer trails; every homing projectile in reach is used
up, whoever it chases. The wheel, the boxes and the coins are not touched,
and with nothing in reach the blast still fires. Every blast is a server
event each client acts on: a flat orange ring grows on the road to the
reach, and the blast is heard by distance (ADR 0076). The drone warning
goes with it: a marker beside the slot and a rising beep while a homing
projectile chasing the own kart is within 40 m, so the blast can be timed
to meet it. In the slot it is a grandstand air horn, scowling.

### spin-out

A kart stopped and spun by an item, a star kart or the runaway wheel for a fixed time
(1 s): dead to its controls and moved by nothing until the time runs out.
Every spin-out costs the kart three coins, or all it carries. Every
spin-out is a server event, and the snapshots carry the time left; clients
only show it.

### bump

The velocity a kart carries away from a kart or tyre wall it touched,
decaying over a fixed time (0.3 s to a third) and then gone. A tyre wall
gives one as a kart standing still would, with the whole of the bounce
to the kart. A wall gives none: it holds the kart at the corner that met
it, with no bounce. The handling adds it to
the kart's own velocity every tick, so a touch throws the pair apart instead
of letting one push the other along; the kart that drove in loses speed and
the kart hit keeps its own. A new bump replaces one under way, never adds
to it. A contact never turns a kart: only steering does. Resolved on the
server; the client's prediction runs the same rule against the other karts'
kinematic stand-ins, and a snapshot carries each kart's bump, so a rewind
takes the server's bump in place of the prediction's own.

### livery

The palette that tells one kart from another, borrowed from a team's colours:
primary, secondary and accent, the three the kart is painted in, and a
key colour for every single-colour use (the minimap, swatches, kart labels).
Eight liveries, assigned in room join order; each comes with its slot's
driver.

### reconciliation

The client replacing its predicted state with a snapshot and replaying the
inputs the server has not yet acknowledged. The visible correction is blended
over up to six frames rather than snapped.

### interpolation

How the client draws the other karts: between the two snapshots it has
received either side of a render clock 100 ms behind the newest, never
past the newest. The own kart's prediction meets them there as kinematic
bodies it cannot push.

### replay

A race as `{seed, track, bots, level, inputs[]}`: enough to rerun it
anywhere, the bots at the bot level it names.
Inputs are run-length encoded into segments, one input per human kart in
grid order for a run of ticks; the bots drive themselves from the seed. A
replay committed under `assets/replays` with the state hashes it reported is
a fixture of the determinism gate.

### state hash

A platform-independent hash of everything in the sim that influences future
ticks. A replay reports it after every 60th tick; the native and wasm builds
must report the same values for the same replay.

## Track

### course

A track as players see it: its name, its flag and the real circuit it is
drawn after, such as Goldstone, after Silverstone. The creator picks the
course on the Title Scene and it is fixed for the room (ADR 0061). The oval
is a track but no course.

### sidecar

The data file exported from Blender next to a track's glTF: the centreline,
checkpoints, item box and boost pad positions, the spawn grid and the tagged
collision meshes. The sim reads the sidecar and never the glTF.

### surface

What is under a point on the track: asphalt, sand, grass, wall, tyres, or
out of world when nothing is below it. Collision meshes in the sidecar are
tagged with the first five; the sim answers with the topmost one under a
point.

### wall

The barrier along each edge of the run-off, the whole way round every
track: the track limit. A kart can leave the asphalt for the sand but never
the course, so no corner can be cut and no checkpoint skipped. A wall holds
a kart at the corner that met it, with no bounce (ADR 0022). It stands 6 m
off the asphalt on Bell Deer and Goldstone and 2 m off it on Monarch (ADR
0064).

### bridge

Where a course crosses itself: one leg rises on an embankment and crosses
the other on a deck, between its own walls, and the other leg runs beneath
it through the underpass, at ground level. The deck, its embankment and
abutments are scenery; a kart rides the raised leg's own asphalt and sand,
and reads its spline progress near its last reading so it keeps to its own
leg through the crossing (ADRs 0046, 0047).

### prop

Scenery a layout declares in its decor and the builder places beyond the
walls: in the glTF only, never in the sidecar, so karts never touch one
and the sim never learns it exists. A prop that must stop a kart becomes a
wall or a tyre wall with its own ADR. A moving prop is moved by the View
from its own clock, as Presentation (ADR 0050).

### wood

A patch of trees of one kind, pine, round or cherry, that the builder
scatters over a disc or along a line from a count and a seed. The count is
a cap: a tree within 3 m of a wall or 5 m of another is dropped, so no tree
is ever placed by hand, and the seed makes every export scatter the same
trees. A prop (ADR 0050).

### Circuit Wheel

The Ferris wheel outside Bell Deer's first curve: a prop whose rim and
sixteen gondolas are nodes of their own in the glTF, which the View turns
from its own clock, each gondola turned back so it hangs level. It carries
no name (ADR 0050).

### arch

The tyre-shaped ring across the middle of Bell Deer's Dunlop curve: a
prop a kart passes under on the climb to the boost pad, lettered with its
sponsor round the belt on both faces. A span, so it is held to the span
rule, not the 8 m one: its underside keeps the gantry's 7 m over the
asphalt and its feet stand on grass beyond the walls (ADR 0050).

### pit building

The row of garages beside the start straight, which the grid faces
through the countdown: a prop with a door and a flag in each livery's key
colour and no number on any door (ADR 0035). Its doors face the pit boxes
where a course has a pit lane (ADR 0081). Inside Bell Deer's straight; outside Goldstone's, longer and white
(ADR 0060).

### tunnel

Monarch's roof over about 150 m of the tunnel bend: a side against the back
of each wall from the grass to the roof, the roof's underside 6 m over the
road so the chase camera passes under it, and a lamp every 12 m along each
side. Scenery only; the walls under it hold the karts (ADR 0064).

### hangars

The row of five airfield hangars outside Goldstone's Hangar Straight: a
prop of barrel-roofed sheds, each with a dark door in the gable toward the
track, held to the 8 m rule like the pit building (ADR 0060).

### airship

The SLIPSTREAM AIRWAYS airship over Bell Deer's east infield: a prop
that flies, so it has no footprint, a single node in the glTF with its
origin at the nose, which the View flies once round its circle every four
minutes from its own clock, nose along the tangent, its lowest point 30 m
over every road under it. Its sponsor rides on both flanks as children of
the node (ADR 0050, ADR 0038).

### helicopter

The BOX BOX BOX LOGISTICS helicopter over Monarch's harbour: a prop that
flies, like the airship, so it has no footprint. Its body is a node in the
glTF with its origin at the nose, which the View flies anticlockwise once
round its circle every two minutes from its own clock, nose along the
tangent and dipped 5 deg; a main rotor and a tail rotor hang under it as
nodes of their own, which the View spins, and its sponsor rides on both
cabin flanks. Its lowest point keeps 30 m over every road under it
(ADR 0050, ADR 0038).

### display jets

The three red jets over Goldstone's hangars: a prop that flies, like the
airship, so it has no footprint. The whole formation, each jet's smoke
ribbon included, is one node in the glTF with its origin at the leader's
nose, which the View flies nose first along a 600 m pass over the hangars,
its flypast, every 30 s from its own clock, against the race, in 6 s,
and hides between passes. Its lowest point keeps 20 m over every road under the pass
(ADR 0050).

### harbour

Monarch's basin of sea water outside the harbour straight and the pool,
close behind their stands, and round the quay and Rascasse: a prop of flat
water a hair over the grass inside a raised stone quay, with yachts moored
stern-to along the quays near the lap. Nothing in it moves, and it is held to the 8 m rule (ADR 0050).

### casino

The grand house outside Monarch's Casino Square, at the crest: a prop of
stucco under copper roofs, two towers and a dome, standing on a terrace that
rises from the grass to the road's height there, its dark door and windows
toward the track. It carries no name, and it is held to the 8 m rule (ADRs
0050, 0065).

### tyre wall

A stack of tyres standing on the asphalt, tagged `tyres`. A kart that
hits one takes a bump straight out from it and loses speed by how square
it hit, so a glancing touch keeps nearly all its speed and a square hit
sends it back the way it came, slower. It never holds a kart and never
spins one out (ADR 0041).

### sausage kerb

A raised kerb on the inside of a corner's apex. A kart on the ground that
drives onto it is launched into a hop that lands straight, losing a share
of its speed and any drift under way with its charge (ADR 0041).

### runaway wheel

A loose wheel that rolls in across the track on a fixed cycle of the race
clock, with the marshal waving hazard flags for a second before each pass
and through it, bounces twice off the walls, landing somewhere the race
seed picks each time, and rolls out through the far wall. A kart it reaches
spins out, by no one (ADRs 0041, 0042, 0045).

### marshal

The figure on a marshal post beside a runaway wheel's crossing. While the
wheel is quiet he sits with a cup of tea; from the flag going up, a second
before a pass, to the end of it he is on his feet waving two hazard flags,
and while the wheel rolls he hops. Presentation only (ADR 0045).

### hazard flag

One of the two flags the marshal waves: yellow with black diagonals, not
a racing flag. Nothing to do with the item called a hazard (ADR 0045).

### hazard zone

The paint over a runaway wheel's crossing: wall to wall and as far along
the track either side as the wheel can reach, yellow and black chevrons
across each end and a light yellow tint between. It flashes red from the
flag going up to the end of the pass. Paint only: not a surface, and not
the item called a hazard.

## Client

### Scene

One screen of the client with its own lifecycle: Title, Room, Race, Results.
Exactly one Scene is active at a time.

### View

The part of a Scene that renders the world. The Race Scene has one View, the
3D track, drawn by Three.js from kart transforms the bridge reports.

### Instrument panel

The HUD overlaid on a View: position, lap, lap time, best lap, item slot,
speed, gear, coins and tyre, wrong-way warning, connection banner, the start
lights over the View and the course map. Reads state; never drives it.

### plate

A readout's chamfered backing on the Instrument panel: a lit, a mid and a
shaded band of one colour, over the ink drop, with no outline (ADR 0037).
Carbon for most readouts, the local kart's primary for the position, the
item box gold for the item slot, a kart's key colour for its label.

### gear

The Instrument panel's reading of the local kart's speed as a gearbox: six
even bands up to top speed, N while the speed shows as 0 km/h, R while
reversing, and 6th held on the limiter over top speed. Worked out in the
client from the kart readout's speed; the sim has no gears (ADR 0031).

### shift lights

The row of twelve lights over the gear, lit green, red then blue in thirds
as the revs in the gear climb to the shift, all blue on the limiter.
Decorative: the gear and the speed are the text.

### tyre life

How much life a kart's tyres have left, as a percentage: 100% is new and 0%
is gone. It falls as the kart is driven and only a stop raises it. The
Instrument panel's tyre gauge shows it on the local kart drawn from above,
its tyres green above 60%, yellow above 30% and red below, read rounded up so
0% means truly gone (ADR 0080). With Tyres Off it holds at 100%. Avoid: tyre
wear, for the number.

### Tyres

Whether a race's tyres wear: On or Off, the creator's pick on the Title Scene,
fixed for the room as the course is and shown on the Room Scene (ADR 0083). Off
is the race as it was before there was tyre life, and what a kept room or a
replay without a pick is. A replay carries it.

### race readout

What the Race Scene reads of a race once a frame, as of the last tick:
every kart readout and standing, the phase, the local kart's item slot, the
item boxes and the coins there to pick up, the hazards and the projectiles.
A solo race reports it from the sim in the browser and a room's race from
the server's snapshots and events; the View, the Instrument panel and the
audio each read the one readout. Not a snapshot, which is the server's message.

### course map

The track's outline seen from above in a corner of the Instrument panel,
drawn from the sidecar's centreline as asphalt between white walls with a
chequered start, and a faceted octagon marker per kart in its key colour at
the transform the View draws that frame. At a bridge the deck is drawn
over the road beneath, so the map reads as a crossing rather than an X. The own kart's marker is larger
and ringed in white. The map is fixed: it never rotates with
the own kart.

### Presentation

Everything the player sees and hears that is not a rule: meshes, materials,
particles, camera motion, audio. Presentation never changes sim state.

### Feedback

A piece of Presentation tied to one mechanic so the mechanic has a visible or
audible tell: tyre marks on drift, sparks on mini-turbo charge, a dust puff
off-track, a spin animation on spin-out, beeps on the countdown and sparks
on a rocket start's charge, a sputter and dust at the wheels on its stall,
a shadow disc on the road under each kart so a hop's height reads, and on
every boost a flame at the exhaust and, for the own kart, wind lines and a
wider field of view; and on the runaway wheel, flames off its back, a smoke
trail, dust where it lands and sparks off each wall it rebounds from.

### head sway

The driver's head rolling at the neck against the lateral G of a turn:
turn left and the head tips right, further the harder the turn up to a
cap. The head holds level through a spin-out or a stall. Presentation
worked out in the client from two kart readouts; the sim has no head.
