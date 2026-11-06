//! Single home for gameplay, item, bot and network constants (ADR 0014).
//! Tuning a value means editing this file and nothing else; no other module
//! may hard-code a number that belongs here. Initial values are the PRD's.

// Rates. Every rule is written per tick; frame rate never enters the sim.

/// Fixed sim rate on both client and server.
pub const TICKS_PER_SECOND: u32 = 60;
pub const SNAPSHOTS_PER_SECOND: u32 = 30;
/// Seconds of one tick: the `dt` every step on either side is run with.
pub const TICK_S: f32 = 1.0 / TICKS_PER_SECOND as f32;
/// How far behind the newest snapshot other karts are rendered.
pub const INTERPOLATION_DELAY_S: f32 = 0.1;
/// The interpolation's clock runs each tick by one tick plus this share of
/// how far it is from its target, so it settles on the delay over a few
/// dozen ticks instead of following every snapshot's jitter.
pub const INTERPOLATION_CLOCK_GAIN: f32 = 0.05;
/// The most the interpolation's clock runs fast or slow, as a share of a
/// tick: a tenth, too little to see in a kart's speed.
pub const INTERPOLATION_CLOCK_SLEW: f32 = 0.1;
/// A kart drawn further than this per tick between two snapshots was put
/// there, by a respawn, and is not drawn in between: twice the biggest
/// boost's top speed, a margin over the fastest a kart drives.
pub const INTERPOLATION_TELEPORT_M_PER_TICK: f32 =
    2.0 * BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S / TICKS_PER_SECOND as f32;
/// Further than this from its target, as after a stalled tab, the
/// interpolation's clock jumps to it rather than taking seconds to slew.
pub const INTERPOLATION_RESYNC_S: f32 = 0.5;
/// Sim ticks a reconciliation correction is blended over: the six frames of
/// ADR 0009 at the sim's rate, so the blend lasts as long on any display.
pub const RECONCILIATION_BLEND_TICKS: u32 = 6;
/// A snapshot that puts the own kart within this many metres of where the
/// client predicted it, and its nose and next tick's travel as close, agrees
/// with the prediction: the difference is rounding, not a correction.
pub const RECONCILIATION_TOLERANCE_M: f32 = 0.001;
/// How far a snapshot's tyre life may sit from the prediction's, in percent,
/// before it rewinds to take the server's word. A touch only the server
/// knows of is not worth one: ADR 0080 sizes this at too little to see.
pub const RECONCILIATION_TYRE_LIFE_TOLERANCE_PERCENT: f32 = 1.0;
/// The largest correction of the own kart a player cannot see in the chase
/// camera: blended over the tuned ticks it moves the kart under 5 cm a
/// tick, against the half metre a tick the kart travels at top speed.
/// The target at 100 ms RTT with 2 percent loss is to stay under it
/// (ADR 0009).
pub const RECONCILIATION_VISIBLE_CORRECTION_M: f32 = 0.25;
/// A replay reports the state hash after every this-many ticks: once a
/// second, the determinism gate's resolution (ADR 0002).
pub const REPLAY_HASH_INTERVAL_TICKS: u32 = TICKS_PER_SECOND;

// Network (ADR 0011).

/// How long the client gives WebTransport to connect before it falls back
/// to WebSocket.
pub const WEBTRANSPORT_CONNECT_TIMEOUT_S: f32 = 3.0;
/// How long a Ping waits for its Pong before the server counts as unreachable.
pub const PING_TIMEOUT_S: f32 = 5.0;
/// How often an unanswered Ping is sent again; a datagram may be lost.
pub const PING_RESEND_S: f32 = 0.5;
/// Silence that counts as a lost connection mid-race (ADR 0012): a client
/// that has heard nothing from the server for this long shows the
/// connection banner and reconnects, and a human the server has heard no
/// input from for this long is handed to a bot.
pub const CONNECTION_SILENCE_S: f32 = 2.0;
/// How long a client waits after a failed reconnect before the next try.
pub const RECONNECT_RETRY_S: f32 = 1.0;
/// How often the server probes each seated player's connection with a
/// Ping on the datagram lane, for the round-trip time and packet loss
/// metrics (ADR 0018). A probe not answered before the next one counts as
/// lost.
pub const CONNECTION_PROBE_INTERVAL_S: f32 = 1.0;
/// How many of the latest probes the packet loss gauge is read over: at a
/// probe a second, about a minute, which resolves the 2 percent loss the
/// reconciliation is tuned for (ADR 0009).
pub const CONNECTION_PROBE_WINDOW: usize = 64;
/// How long a client shows the fire it predicted from a press before it
/// takes the server not to have fired, because the slot was already empty
/// there. Longer than a round trip, so a press whose datagram was lost and
/// fired from the next one still confirms in time.
pub const PREDICTED_FIRE_TIMEOUT_S: f32 = 1.0;
/// Snapshots an event is repeated in after the first that carries it, so a
/// lost datagram or two still leaves a client with every event; the client
/// keeps the first copy of each id.
pub const SNAPSHOT_EVENT_REPEATS: usize = 2;
/// How far, in ticks, the sequence the server applies may run from the
/// newest input it has received for a human: further behind and it drops
/// inputs to catch up, so a client whose clock runs fast adds no more delay;
/// further ahead and it waits, so a client back from a stall is heard again.
pub const INPUT_QUEUE_MAX_TICKS: u32 = 3;
/// Inputs the server holds for a human before it applies the first, so the
/// queue starts one input ahead and a datagram that lands a little late is
/// still in time.
pub const INPUT_QUEUE_START_TICKS: usize = 2;
/// The longest message one frame of the reliable stream may carry; a frame
/// claiming more is not a client of ours and ends the session.
pub const RELIABLE_FRAME_MAX_BYTES: u32 = 64 * 1024;

// Rooms (ADR 0012).

/// Letters a room code is drawn from: the alphabet without I, L and O,
/// which read as digits in a chat message or over a shoulder.
pub const ROOM_CODE_ALPHABET: &str = "ABCDEFGHJKMNPQRSTUVWXYZ";
pub const ROOM_CODE_LENGTH: usize = 4;
/// Players a room holds; every human gets a kart, so no more than the grid.
pub const ROOM_PLAYERS_MAX: usize = KARTS_PER_RACE;
/// Seconds after the last human leaves before a room enters Closing.
pub const ROOM_CLOSE_AFTER_EMPTY_S: f32 = 300.0;
/// Rooms the server hosts at once; a create past this is refused until one
/// closes (ADR 0073). A guess at what one t4g.small races, not a measurement.
pub const ROOMS_MAX: usize = 32;
/// Rooms one address may create per `ROOM_CREATE_WINDOW_S` (ADR 0073). Empty
/// rooms stand for `ROOM_CLOSE_AFTER_EMPTY_S`, so one address can hold this
/// many per window in that time: under half of `ROOMS_MAX`.
pub const ROOM_CREATES_PER_ADDRESS_MAX: usize = 3;
pub const ROOM_CREATE_WINDOW_S: f32 = 60.0;
/// Seconds a room brought back after a server restart (ADR 0005) holds
/// each seat for its player's rejoin before freeing it: long enough for
/// every client's reconnect, which retries each `RECONNECT_RETRY_S`
/// through a restart of under ten seconds, and short enough that a seat
/// whose player closed the tab does not hold up the next race.
pub const RESTORED_SEAT_GRACE_S: f32 = 30.0;
/// Bounds on a nickname's length in characters.
pub const NICKNAME_MIN_CHARS: usize = 2;
pub const NICKNAME_MAX_CHARS: usize = 12;

// Race.

/// Karts on the grid; empty slots are filled by bots at Start.
pub const KARTS_PER_RACE: usize = 8;
/// Laps in a race nobody picked for: a kept room or a replay without a pick,
/// whatever the Title Scene offers first (ADR 0084).
pub const DEFAULT_LAPS: u32 = 3;
/// The laps a creator can pick from, the default first. Five is long enough
/// that one stop beats none (ADR 0083).
pub const LAP_CHOICES: [u32; 2] = [DEFAULT_LAPS, 5];
/// Seconds the karts are held on the grid before Go: the start lights, one
/// a second, all five out at Go. The Instrument panel always draws five
/// lamps, so this must stay one second per lamp.
pub const COUNTDOWN_S: f32 = 5.0;
/// Seconds after the first human's finish before the race ends whoever is
/// still racing, with those karts placed by progress (ADR 0032).
pub const RACE_END_CAP_S: f32 = 60.0;
/// Seconds the Race View holds once the race has ended, every kart driven
/// by a bot, before the Results Scene opens.
pub const RACE_OUTRO_S: f32 = 3.0;
/// Seconds of decreasing spline progress before the wrong-way warning.
pub const WRONG_WAY_WARNING_S: f32 = 2.0;
/// Metres either side of the last spline progress that the next reading
/// searches, wrapping past the start line (ADR 0046). It must be wider
/// than the furthest a kart or projectile travels in one tick, and than a
/// kerb a bot steers for lies ahead of the bot. It must be narrower than
/// the lap distance between two legs of a course that cross, or a reading
/// at the crossing can land on the other leg.
pub const PROGRESS_WINDOW_M: f32 = 40.0;

// World. Metres and seconds. The kart's collider is a box of these
// dimensions; the View draws the same box until the glTF kart lands (ADR 0007).

pub const GRAVITY_M_PER_S2: f32 = -9.81;
/// Half the kart's length, nose to tail along its facing.
pub const KART_HALF_LENGTH_M: f32 = 0.9;
/// Half the kart's width, wheel to wheel.
pub const KART_HALF_WIDTH_M: f32 = 0.6;
/// Half the kart's height; a kart at rest sits this far above the surface.
pub const KART_HALF_HEIGHT_M: f32 = 0.3;
/// How much further than its resting height the surface under a kart's
/// centre may lie while the kart still counts as on the ground. A kart
/// stays level, so on a ramp it rests on its uphill edge with its centre
/// raised over the road: room for the bridge's 14 percent grade, 0.13 m
/// over the kart's half length, with some to spare.
pub const GROUND_CONTACT_M: f32 = 0.2;
/// Rounding on the kart's collision box, inside every half extent. A
/// sharp-edged box catches on the seams between track triangles; a rounded
/// one slides over them.
pub const KART_EDGE_RADIUS_M: f32 = 0.05;

// Handling. Speeds are metres per second; boosts scale top speed.

pub const TOP_SPEED_M_PER_S: f32 = 30.0;
pub const ACCELERATION_TO_TOP_S: f32 = 2.5;
pub const STEER_RATE_LOW_SPEED_RAD_PER_S: f32 = 2.2;
pub const STEER_RATE_TOP_SPEED_RAD_PER_S: f32 = 1.2;
/// Seconds from top speed to a standstill under full brake.
pub const BRAKE_TO_STOP_S: f32 = 1.5;
/// A brake held on its own past the standstill backs the kart up, at the
/// brake's rate, no faster than this.
pub const REVERSE_TOP_SPEED_M_PER_S: f32 = 8.0;
/// Seconds from top speed to a standstill with no throttle and no brake.
pub const COAST_TO_STOP_S: f32 = 4.0;
/// Below this speed a kart barely steers, so it never pivots in place.
pub const STEER_FULL_EFFECT_SPEED_M_PER_S: f32 = 1.0;
pub const HOP_AIRTIME_S: f32 = 0.3;
/// A landing hop starts a drift only above this fraction of top speed.
pub const DRIFT_ENTRY_SPEED_FRACTION: f32 = 0.4;
/// How hard a drift turns with the stick centred, as a multiple of full
/// steer lock off a drift.
pub const DRIFT_TURN_NEUTRAL: f32 = 1.0;
/// How much full inward steer tightens a drift's turn, and full
/// counter-steer widens it, as a multiple of full lock. Less than the
/// neutral strength so counter-steer never turns the kart the other way.
pub const DRIFT_TURN_STEER_RANGE: f32 = 0.5;
/// How much a full brake tightens a drift's turn on top of the steer, as a
/// multiple of full lock: the brake drift. A partial brake tightens in
/// proportion.
pub const DRIFT_TURN_BRAKE: f32 = 0.5;
/// Radians between where a drifting kart points and where it travels; the
/// nose leads into the corner by this much.
pub const DRIFT_SLIP_ANGLE_RAD: f32 = 0.3;
pub const MINI_TURBO_ORANGE_CHARGE_S: f32 = 0.3;
pub const MINI_TURBO_ORANGE_BOOST_S: f32 = 0.6;
pub const MINI_TURBO_RED_CHARGE_S: f32 = 0.6;
pub const MINI_TURBO_RED_BOOST_S: f32 = 1.0;
pub const MINI_TURBO_BOOST_FACTOR: f32 = 1.3;
pub const BOOST_PAD_S: f32 = 1.0;
pub const BOOST_PAD_FACTOR: f32 = 1.4;
/// The pad is a rectangle on the surface, this big either side of its
/// placement along and across the direction it faces; a kart whose centre
/// is on it while on the ground is boosted. Narrower than the asphalt, so a
/// pad can be missed on purpose.
pub const BOOST_PAD_HALF_LENGTH_M: f32 = 2.0;
pub const BOOST_PAD_HALF_WIDTH_M: f32 = 3.0;
/// A sausage kerb is a rectangle on the surface, this big either side of
/// its placement along and across the direction it faces; a kart whose
/// centre reaches it on the ground is launched into a hop that lands
/// straight.
pub const KERB_HALF_LENGTH_M: f32 = 4.0;
pub const KERB_HALF_WIDTH_M: f32 = 0.5;
/// The share of a kart's speed the kerb's launch leaves it.
pub const KERB_SPEED_KEEP: f32 = 0.85;
/// The runaway wheel (GLOSSARY) makes a pass over its crossing once every
/// period, each pass rolling in from the other wall from the last, the
/// first starting at `WHEEL_FIRST_PASS_S` on the race clock so the
/// leader's first lap meets a quiet approach. The marshal's flag is up from
/// `WHEEL_WARNING_S` before a pass until it ends.
pub const WHEEL_PERIOD_S: f32 = 10.0;
pub const WHEEL_WARNING_S: f32 = 1.0;
pub const WHEEL_FIRST_PASS_S: f32 = 20.0;
/// A pass rolls at this speed throughout, rebounds off the walls this many
/// times and leaves through the far wall; each wall to wall leg ends at a
/// point drawn from the race seed within the spread either side of the
/// crossing's line, along the track. A track whose crossing is too wide for
/// the longest pass and its warning to fit in a period does not load.
pub const WHEEL_SPEED_M_PER_S: f32 = 15.0;
pub const WHEEL_BOUNCES: u32 = 2;
pub const WHEEL_LANDING_SPREAD_M: f32 = 8.0;
/// A kart whose collision box comes within this much of the wheel's centre,
/// seen from above, is reached by it, as by a hazard. The View draws the
/// wheel this size, so the wheel seen is the wheel that hits; 2.4 m across
/// reads as a danger from the chase camera (ADR 0044).
pub const WHEEL_RADIUS_M: f32 = 1.2;

// The slipstream (GLOSSARY: slipstream). A kart that holds another's wake
// for the charge is towed: its top speed cap rises for as long as it stays
// there and for the tow's time after it pulls out. No item and no button,
// open to every kart and every bot (ADR 0077).

/// How far behind a kart, centre to centre along its travel, its wake
/// reaches. The bots' lookahead, so a bot tucked in behind the kart it
/// steers round is in its wake.
pub const SLIPSTREAM_RANGE_M: f32 = BOT_LOOKAHEAD_M;
/// How far either side of the leader's travel the follower's centre may
/// sit: twice a kart's half width, so a kart half a kart off line is still
/// in the wake.
pub const SLIPSTREAM_HALF_WIDTH_M: f32 = 2.0 * KART_HALF_WIDTH_M;
/// The most the two karts' travel directions may differ.
pub const SLIPSTREAM_ANGLE_RAD: f32 = 0.5;
/// A kart below this share of the tuned top speed leaves no wake, so a
/// spun-out, stalled or stopped kart tows nobody.
pub const SLIPSTREAM_LEADER_SPEED_FRACTION: f32 = 0.5;
/// Continuous time in a wake before the tow takes. Counted in ticks, so no
/// tick is paid late as an f32 sum would.
pub const SLIPSTREAM_CHARGE_S: f32 = 1.0;
pub const SLIPSTREAM_CHARGE_TICKS: u32 =
    (SLIPSTREAM_CHARGE_S * TICKS_PER_SECOND as f32 + 0.5) as u32;
/// How long the tow holds after the kart leaves the wake.
pub const SLIPSTREAM_TOW_S: f32 = 1.0;
/// What the tow multiplies the top speed cap by. A cap and not a boost, so
/// the brake still works and a mini-turbo under way is never replaced.
pub const SLIPSTREAM_TOP_SPEED_FACTOR: f32 = 1.15;

// Rocket start (GLOSSARY). What the throttle hold ending at Go earns is
// read from the hold's length; a release before Go resets it. The edges
// approximate Mario Kart 8 Deluxe as its guides describe the timing, with
// Mario Kart Wii's data-mined 2.0 s burnout for the stall.

/// The throttle counts as held above this, so a finger resting on an
/// analog trigger never stalls the kart.
pub const START_THROTTLE_HELD: f32 = 0.5;
/// A hold longer than this, begun before the fourth light came on, stalls
/// the kart.
pub const START_STALL_HOLD_S: f32 = 2.0;
/// A hold at least this long, begun in the first half of the fourth light,
/// earns the full boost.
pub const START_FULL_BOOST_HOLD_S: f32 = 1.5;
/// A hold at least this long, begun in the second half of the fourth light,
/// earns the weak boost; a shorter one, begun at the fifth light or later,
/// starts clean.
pub const START_WEAK_BOOST_HOLD_S: f32 = 1.0;
/// The full boost: the boost item's values.
pub const START_BOOST_S: f32 = ITEM_BOOST_S;
pub const START_BOOST_FACTOR: f32 = ITEM_BOOST_FACTOR;
/// The weak boost: the orange mini-turbo's values.
pub const START_WEAK_BOOST_S: f32 = MINI_TURBO_ORANGE_BOOST_S;
pub const START_WEAK_BOOST_FACTOR: f32 = MINI_TURBO_BOOST_FACTOR;
/// Seconds a stalled kart sits dead to every control after Go, as in a
/// spin-out.
pub const START_STALL_S: f32 = 2.0;
/// Top speed fraction on sand and grass.
pub const OFF_TRACK_SPEED_FACTOR: f32 = 0.55;
/// Seconds off the course, outside the world bounds or behind a wall,
/// before respawn at the last checkpoint.
pub const RESPAWN_DELAY_S: f32 = 1.5;

// Kart contact. Two karts that touch bounce apart: each takes a bump, a
// velocity away from the other that the handling adds to its own and that
// dies away over the tuned time, and the one that drove into the other
// loses speed. Karts never push each other along.

/// Metres per second the least a touch pushes each kart away from the
/// other, so karts rubbing side by side part.
pub const KART_BUMP_MIN_M_PER_S: f32 = 2.0;
/// The share of the closing speed the pair bounces apart with, on top of
/// the least bump, half to each kart: at 1 the two fly apart as fast as
/// they met, at 0 they only stop closing. With the speed the driver loses
/// on top, 1 gives a kart rammed square a short recoil.
pub const KART_BUMP_RESTITUTION: f32 = 1.0;
/// Seconds for a bump to decay to a third; it is gone within a few.
pub const KART_BUMP_DECAY_S: f32 = 0.3;
/// The share of the closing speed a kart driving square into another
/// loses from its own; a glancing touch loses the share times how square
/// it was, and a kart hit from behind or beside loses none.
pub const KART_BUMP_SPEED_LOSS: f32 = 0.5;

// Tyre walls. A kart that hits one bounces off as off a kart standing still:
// a bump straight out from the stack, decaying as a kart bump does, and a
// loss of speed scaled by how square it hit. A wall holds; a tyre wall
// never does.

/// The share of the closing speed a kart bounces off a tyre wall with, on
/// top of the least bump. A touch between karts shares its bounce; a tyre
/// wall gives the kart the whole of it.
pub const TYRE_WALL_RESTITUTION: f32 = 0.4;
/// The share of the closing speed a kart driving square into a tyre wall
/// loses from its own; a glancing touch loses the share times how square
/// it was.
pub const TYRE_WALL_SPEED_LOSS: f32 = 0.7;

// Items.

/// Seconds after a box is picked up before it is back to pick up again.
pub const ITEM_BOX_RESPAWN_S: f32 = 3.0;
/// Seconds an item rolls in the slot after a box before it lands and can be
/// fired. The slot counts as full while it rolls.
pub const ITEM_ROLL_S: f32 = 2.5;
/// A box is picked up by a kart whose centre is within this much of its
/// placement along the direction it faces and across it. The boxes of a
/// row stand 3 m apart, so the widths meet without overlapping and a kart
/// between two takes one.
pub const ITEM_BOX_HALF_LENGTH_M: f32 = 1.0;
pub const ITEM_BOX_HALF_WIDTH_M: f32 = 1.5;
pub const ITEM_BOOST_S: f32 = 1.5;
pub const ITEM_BOOST_FACTOR: f32 = 1.4;
/// Metres behind the kart a hazard is dropped.
pub const HAZARD_DROP_DISTANCE_M: f32 = 1.0;
/// A kart whose collision box comes within this much of a hazard's centre
/// hits it. A fresh drop lies within its owner's reach; the owner is spared
/// by having to get clear of it first, not by this.
pub const HAZARD_RADIUS_M: f32 = 0.5;
/// Seconds a dropped hazard lies on the track before it is gone; a held
/// hazard does not age.
pub const HAZARD_LIFETIME_S: f32 = 30.0;
/// Seconds a kart hit by an item is stopped, with its inputs ignored.
pub const SPIN_OUT_S: f32 = 1.0;
/// Homing projectile speed as a multiple of top speed: faster than any
/// boost, so a target cannot outrun it.
pub const HOMING_SPEED_FACTOR: f32 = 1.6;
/// Metres ahead on the centreline a projectile steers for while it
/// travels the spline, so it merges onto the line from wherever it was
/// fired instead of snapping to it.
pub const HOMING_LOOKAHEAD_M: f32 = 8.0;
/// Within this many metres of its target a projectile leaves the spline
/// and flies straight at the kart, so a kart off the centreline is hit
/// rather than passed.
pub const HOMING_LOCK_RANGE_M: f32 = 10.0;
/// A projectile whose centre comes within this much of its target's
/// collision box hits it.
pub const HOMING_RADIUS_M: f32 = 0.5;
/// What a homing projectile fired with no kart ahead, the leader's, does:
/// it is wasted, the fire spawning nothing. The alternative, a projectile
/// that circles the lap until a kart is ahead of it, is not taken, since
/// it would let the leader mine the track for whoever passes. The sim
/// builds only the wasted fire and refuses to compile if this is flipped.
pub const HOMING_WITHOUT_TARGET_IS_WASTED: bool = true;

// The star (GLOSSARY: star). Fired, it gives the kart a run: a raised top
// speed and immunity, spinning out every kart it touches, for a fixed time
// that a second star puts back to the start without adding to it.

/// Seconds a run lasts from the fire. assets/audio/build.py renders the
/// star music to the same length from its own copy, `STAR_RUN_S`, so a
/// retune here moves it there and rebuilds the music.
pub const STAR_S: f32 = 6.0;
/// What a run multiplies the kart's top speed cap by, on top of its coins
/// and its rubber band. A cap and not a boost, so the brake still works.
pub const STAR_TOP_SPEED_FACTOR: f32 = 1.25;
/// The kick on firing: a boost of this length at the boost item's factor,
/// so the fire is felt as a jump before the cap takes over.
pub const STAR_KICK_S: f32 = 0.5;
pub const STAR_KICK_FACTOR: f32 = ITEM_BOOST_FACTOR;
/// A kart whose collision box comes within this much of a star kart's
/// centre is hit by it. The star kart's collider ignores other karts for
/// the run, so this reach is the touch.
pub const STAR_HIT_RADIUS_M: f32 = 1.0;

// The Air Horn (GLOSSARY: Air Horn). Fired, it lets off one blast round the
// kart that spins out the karts in reach and uses up the hazards and
// projectiles in reach; the one item worth saving and timing.

/// How far from the firing kart's centre the blast reaches, seen from
/// above: a kart whose collision box, a hazard or a projectile within this
/// is in reach. Wall to wall on a road about 12 m wide.
pub const HORN_RADIUS_M: f32 = 7.0;
/// The most spline progress away, the shorter way round the lap, that the
/// blast reaches, so distance alone does not carry it through a crossover
/// or across the barrier between two sections of road.
pub const HORN_PROGRESS_WINDOW_M: f32 = 15.0;

// Coins (GLOSSARY: coin). A kart collects the coins lying on the track, up
// to a full purse, each raising its forward top speed; every spin-out costs it some.

/// What each coin a kart carries adds to its forward top speed, as a share
/// of the tuned top speed: a full purse raises 30 m/s to 33. Acceleration,
/// reverse and handling are untouched.
pub const COIN_TOP_SPEED_FACTOR: f32 = 0.01;
/// The most coins a kart carries; with a full purse it still takes the
/// coins it drives over, but its count holds.
pub const COIN_CAP: u32 = 10;
/// Coins a spin-out costs a kart, whatever spun it out; a kart carrying
/// fewer loses them all. Lost coins vanish.
pub const COINS_LOST_PER_SPIN_OUT: u32 = 3;
/// The life of new tyres, as a percentage (ADR 0080).
pub const TYRE_LIFE_NEW_PERCENT: f32 = 100.0;
/// The life at which the cliff starts: the steep column of every table below,
/// the gauge's red, the BOX call, and a worn kart's smoke and darker marks.
pub const TYRE_LIFE_CLIFF_PERCENT: f32 = 30.0;
/// The lives the tables below are tuned at, new first; between two the
/// handling reads in a straight line (ADR 0080).
pub const TYRE_LIFE_COLUMNS_PERCENT: [f32; 4] =
    [TYRE_LIFE_NEW_PERCENT, 60.0, TYRE_LIFE_CLIFF_PERCENT, 0.0];
/// What those lives leave of the top speed cap, the steering rate, the
/// acceleration and the braking. The cap falls steeply enough to read on
/// the speedo before the gauge turns red (ADR 0087). The steering rate
/// stays ADR 0080's: a bot shares it, and bots on a steeper one wore to 0%
/// and then fell past the race's end cap and onto Monarch's grass.
pub const TYRE_TOP_SPEED_FACTORS: [f32; 4] = [1.0, 0.97, 0.88, 0.70];
pub const TYRE_STEER_RATE_FACTORS: [f32; 4] = [1.0, 0.97, 0.88, 0.65];
pub const TYRE_ACCELERATION_FACTORS: [f32; 4] = [1.0, 0.97, 0.88, 0.70];
pub const TYRE_BRAKING_FACTORS: [f32; 4] = [1.0, 0.98, 0.90, 0.75];
/// The most, in radians, those lives let the line of travel slide wide of
/// the facing. Held at 0% where a bot, which does not foresee the slide,
/// still makes the turn into the pit lane at its approach speed (ADR 0087).
pub const TYRE_WASH_OUT_MOST_RAD: [f32; 4] = [0.0, 0.0, 0.10, 0.15];
/// A worn kart holds its line up to this speed times yaw rate, in m/s^2, and
/// slides all the worn tyres allow from the full load. Full lock flat out
/// is about 30.7 at 30% and 20.5 at 0%, where the worn cap holds the load
/// short of the full one, so past the cliff it slides as far as the table
/// says and worn right out most of it; the lane's approach at 20 m/s on
/// tyres worn right out loads 19.9, and a gentle steer never slides.
pub const TYRE_WASH_OUT_LIMIT_M_PER_S2: f32 = 16.0;
pub const TYRE_WASH_OUT_FULL_M_PER_S2: f32 = 22.0;
/// How fast a slide builds and lets go, so a keyed steer, all or nothing,
/// bends the line rather than kinking it: the deepest in a quarter second,
/// and a hop's airtime lets the deepest go.
pub const TYRE_WASH_OUT_RATE_RAD_PER_S: f32 = 0.6;
/// Life lost per second rolling on the ground.
pub const TYRE_WEAR_ROLLING_PER_S: f32 = 0.06;
/// Life lost per second cornering at [`TYRE_WEAR_CORNERING_REFERENCE_M_PER_S2`];
/// it goes with the square of the kart's speed times its yaw rate.
pub const TYRE_WEAR_CORNERING_PER_S: f32 = 0.19;
pub const TYRE_WEAR_CORNERING_REFERENCE_M_PER_S2: f32 = 20.0;
/// Life lost per second drifting, on top of cornering.
pub const TYRE_WEAR_DRIFT_PER_S: f32 = 0.8;
/// Life each paid mini-turbo release costs.
pub const TYRE_WEAR_MINI_TURBO_PERCENT: f32 = 0.5;
/// Life lost per second at full brake, scaled by the brake input.
pub const TYRE_WEAR_BRAKING_PER_S: f32 = 0.25;
/// Life lost per second on grass or sand.
pub const TYRE_WEAR_OFF_TRACK_PER_S: f32 = 1.25;
/// Life each spin-out costs, whatever caused it.
pub const TYRE_WEAR_SPIN_OUT_PERCENT: f32 = 4.0;
/// Life a touch with a kart or tyre wall costs: the least, then this much
/// per metre per second of closing speed, up to the most.
pub const TYRE_WEAR_BUMP_MIN_PERCENT: f32 = 0.3;
pub const TYRE_WEAR_BUMP_PER_M_PER_S: f32 = 0.05;
pub const TYRE_WEAR_BUMP_MAX_PERCENT: f32 = 2.5;
/// Life lost per second held against a wall.
pub const TYRE_WEAR_WALL_PER_S: f32 = 1.0;
/// Life each sausage kerb launch costs.
pub const TYRE_WEAR_KERB_PERCENT: f32 = 1.0;
/// The pit limiter's speed, from the entry line to the exit line of a pit
/// lane: 54 km/h. It beats coins, the slipstream, a star and every boost
/// (ADR 0081).
pub const PIT_LIMIT_M_PER_S: f32 = 15.0;
/// A kart in the lane on Tyres On is taken onto its mark as it comes within
/// this many metres of its pit box along the lane (ADR 0081).
pub const PIT_BOX_CATCH_M: f32 = 7.5;
/// How long the guide onto the mark takes, the kart dead to its controls
/// and touching nothing.
pub const PIT_BOX_GUIDE_S: f32 = 0.5;
/// The stop's roll (ADR 0082): each [`crate::StopTier`]'s chance as a share
/// of 1, in tier order. Judgement, not measured; they sum to 1.
pub const STOP_ROLL_CHANCES: [f32; 5] = [0.05, 0.65, 0.20, 0.08, 0.02];
/// The stationary time each tier draws, from the first second to the last,
/// in tier order; each tier's times sit above the last tier's.
pub const STOP_ROLL_STATIONARY_S: [[f32; 2]; 5] =
    [[1.9, 2.2], [2.3, 3.0], [3.1, 4.5], [5.0, 7.0], [9.0, 12.0]];
/// A coin is collected by a kart whose centre is within this much of its
/// placement along the direction it faces and across it, airborne or not.
pub const COIN_HALF_LENGTH_M: f32 = 1.0;
pub const COIN_HALF_WIDTH_M: f32 = 1.0;
/// Seconds after a coin is collected before it is back: the item box's.
pub const COIN_RESPAWN_S: f32 = ITEM_BOX_RESPAWN_S;

/// What `coins` carried multiply forward top speed by.
pub const fn coin_factor(coins: u32) -> f32 {
    1.0 + coins as f32 * COIN_TOP_SPEED_FACTOR
}

/// What a full purse multiplies forward top speed by.
const COIN_CAP_FACTOR: f32 = coin_factor(COIN_CAP);

/// The forward top speed on each count of coins carried, from none to a
/// full purse, so the Instrument panel cuts its gears without repeating the rule.
const COIN_TOP_SPEEDS_M_PER_S: [f32; COIN_CAP as usize + 1] = {
    let mut speeds = [0.0; COIN_CAP as usize + 1];
    let mut coins = 0;
    while coins <= COIN_CAP {
        speeds[coins as usize] = TOP_SPEED_M_PER_S * coin_factor(coins);
        coins += 1;
    }
    speeds
};

/// Item roll weights for one band of race positions, in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemRollWeights {
    /// First and last race position (1-based, inclusive) this band covers.
    pub positions: (u8, u8),
    pub hazard: u8,
    pub boost: u8,
    pub homing: u8,
    pub star: u8,
    pub horn: u8,
}

impl ItemRollWeights {
    /// What the band's weights sum to.
    const fn total(&self) -> u32 {
        self.hazard as u32
            + self.boost as u32
            + self.homing as u32
            + self.star as u32
            + self.horn as u32
    }
}

/// What each band's weights sum to: a roll in `0..ITEM_ROLL_PERCENT` picks
/// one item.
pub const ITEM_ROLL_PERCENT: u32 = 100;

/// Roll weights by race position. Bands are contiguous and cover every grid
/// slot; each band's weights sum to `ITEM_ROLL_PERCENT`. The star never
/// reaches the front two and is most likely at the back: the comeback item.
/// The horn mostly goes to the front and the middle, which have the most
/// to protect and the most karts nearby.
pub const ITEM_ROLL_WEIGHTS: [ItemRollWeights; 3] = [
    ItemRollWeights {
        positions: (1, 2),
        hazard: 60,
        boost: 15,
        homing: 10,
        star: 0,
        horn: 15,
    },
    ItemRollWeights {
        positions: (3, 5),
        hazard: 25,
        boost: 30,
        homing: 25,
        star: 5,
        horn: 15,
    },
    ItemRollWeights {
        positions: (6, 8),
        hazard: 5,
        boost: 35,
        homing: 40,
        star: 15,
        horn: 5,
    },
];

// Bots. The constants that vary by bot level (ADR 0078) are Basic's here
// by name and the other levels' in `BOT_LEVELS` below.

/// Metres ahead on the centreline spline a bot steers toward.
pub const BOT_LOOKAHEAD_M: f32 = 12.0;
/// Seconds after its item lands in the slot by which a Basic bot has used
/// it; it fires sooner on a straight, and keeps a boost longer while the
/// boost would carry it into the wheel.
pub const BOT_ITEM_USE_DELAY_S: f32 = 2.0;
/// Seconds a bot trails a fired hazard behind itself before dropping it;
/// no longer than it would hold an unused item.
pub const BOT_HAZARD_HOLD_S: f32 = 1.5;
/// Seconds a bot keeps a horn with no kart its blast can hit before it
/// fires it anyway, in place of the first-straight rule and the use delay:
/// the horn is worth saving for a kart to hit.
pub const BOT_HORN_HOLD_S: f32 = 10.0;
/// Top speed scale by distance to the nearest human: slower far ahead,
/// faster far behind. The minimum is Basic's; the maximum is every
/// level's.
pub const BOT_RUBBER_BAND_MIN_FACTOR: f32 = 0.92;
pub const BOT_RUBBER_BAND_MAX_FACTOR: f32 = 1.08;
/// Metres along the spline from the nearest human at which the rubber band
/// reaches its full effect; the scale is linear in between.
pub const BOT_RUBBER_BAND_RANGE_M: f32 = 100.0;
/// A bot hops into a drift when the centreline at its lookahead curves
/// tighter than this, in radians per metre: a turn of 33 m radius or less.
pub const BOT_DRIFT_CURVATURE_RAD_PER_M: f32 = 0.03;
/// A Basic bot slows for a corner to this fraction of the fastest speed
/// full steer lock holds the corner's radius at. Below 1, so a human can
/// carry more speed through a corner than a Basic bot; an Ace bot's is 1.
pub const BOT_CORNER_SPEED_FRACTION: f32 = 0.85;
/// How far ahead a bot looks for a corner to slow for, in seconds of its
/// current speed.
pub const BOT_BRAKE_HORIZON_S: f32 = 1.0;
/// Metres between the centreline samples a bot checks for a corner ahead.
/// Finer than any corner's arc, so no corner is skipped.
pub const BOT_CORNER_SAMPLE_M: f32 = 2.0;
/// Metres right of the centreline a bot's line runs; every tyre wall stands
/// left of it.
pub const BOT_LINE_RIGHT_OF_CENTRE_M: f32 = 2.5;
/// Metres between a sausage kerb's near edge, at its middle, and the line
/// a bot aims along while the kerb is in range: wide enough
/// that pursuit's cut toward the apex misses the kerb's ends, which a
/// straight kerb on a bend sets nearer the line. On Bell Deer the line
/// is the centreline past both kerbs.
pub const BOT_KERB_GAP_M: f32 = 4.4;
/// Metres short of a sausage kerb's near end from which a bot aims along
/// the line clear of it, giving up any pass: far enough that a bot out wide
/// passing a kart on the kerb's side is back on the line before pursuit
/// cuts toward the apex.
pub const BOT_KERB_APPROACH_M: f32 = 2.0 * BOT_LOOKAHEAD_M;
/// Seconds a bot keeps between itself and a runaway wheel's pass through
/// its lane, either side: the throttle it holds and the line it weaves
/// toward bring it there sooner than its speed now says.
pub const BOT_WHEEL_MARGIN_S: f32 = 0.3;
/// The fraction of flat-out acceleration a bot counts on gaining from a
/// standstill when it judges whether a runaway wheel's pass meets it:
/// traffic can hold it back, but it does set off.
pub const BOT_WHEEL_SET_OFF_FRACTION: f32 = 0.5;
/// The least a bot steers into a corner it hops for, so the hop fixes the
/// drift toward the corner even when its line asks for less.
pub const BOT_HOP_STEER: f32 = 0.25;
/// A kart ahead within the lookahead and this far either side of a bot's
/// travel is in its way: a kart's width with room to spare.
pub const BOT_AVOID_HALF_WIDTH_M: f32 = 2.0;
/// Centre to centre, how far beside a kart in its way a bot aims to pass.
pub const BOT_PASS_GAP_M: f32 = 2.5;
/// Under this speed a bot counts as stalled: held by a wall or a kart. Over
/// the 3 m/s kick a kart pinned on a wall takes every 2 s or so, or the stall
/// never lasts long enough to back out of.
pub const BOT_STALL_SPEED_M_PER_S: f32 = 3.2;
/// Seconds stalled before a bot backs up. Longer than a standing start
/// takes to get going, so no bot reverses off the grid.
pub const BOT_STALL_S: f32 = 1.0;
/// Seconds a bot backs up before it drives on again.
pub const BOT_REVERSE_S: f32 = 1.0;
/// The chance a Basic bot rolls the full boost at Go, from the race's seed
/// once per race; otherwise it starts clean. A bot never stalls.
pub const BOT_START_BOOST_CHANCE: f32 = 0.5;
/// Points of tyre life a bot's pit call is off by, either way, from a hash of
/// the seed and its slot, so a level's bots do not all pit on one lap (ADR 0081).
pub const BOT_PIT_CALL_VARIES_BY: f32 = 5.0;
/// Metres short of the fork that a bot makes its pit call and turns for the
/// lane, and the nearest it will: inside that it could no longer get across.
pub const BOT_PIT_CALL_FROM_M: f32 = 35.0;
pub const BOT_PIT_CALL_UNTIL_M: f32 = 10.0;
/// The speed a bot called in slows to for the fork, where it has a lane's
/// width to find the road and a wall behind it.
pub const BOT_PIT_APPROACH_M_PER_S: f32 = 20.0;
/// Laps of the race a bot must have worn tyres over before it trusts its
/// rate of wear, so a kart handed over mid-stint projects nothing from nothing.
pub const BOT_PIT_LEAST_STINT_LAPS: f32 = 0.25;
/// Seconds an Ace bot reckons a whole stop costs (ADR 0081).
pub const BOT_PIT_STOP_COST_S: f32 = 8.0;
/// `[tyre life, seconds a lap lost at it]`, new tyres first: the lap costs an
/// Ace bot's projection weighs a stop against, measured by a lone Ace bot held
/// at each life on the courses with a lane (ADR 0087). The sim's ignored
/// measurement test refuses a row a second off what it measures.
pub const BOT_TYRE_LOSS_S_PER_LAP: [[f32; 2]; 7] = [
    [100.0, 0.0],
    [60.0, 0.3],
    [40.0, 4.6],
    [30.0, 5.0],
    [20.0, 8.1],
    [10.0, 15.0],
    [0.0, BOT_TYRE_LOSS_WORN_OUT_S_PER_LAP],
];
/// The table's last row: seconds a lap lost on tyres worn right out.
pub const BOT_TYRE_LOSS_WORN_OUT_S_PER_LAP: f32 = 17.4;

/// How long a bot that rolled the boost holds the throttle before Go:
/// midway through the full boost's window, clear of both edges.
pub const BOT_START_HOLD_S: f32 = (START_FULL_BOOST_HOLD_S + START_STALL_HOLD_S) / 2.0;

/// When a bot calls itself in for new tyres (ADR 0081), read at each lane
/// approach with a lap or more to go.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BotPitCall {
    /// Its life is `life` percent or less; it often overstays and lives with it.
    AtLife { life: f32 },
    /// Its life is `life` percent or less, or would run out before the next
    /// lane, a lap on.
    AtLifeOrBeforeItRunsOut { life: f32 },
    /// Its life, carried to the flag at the rate it has worn so far, ends under
    /// `flag_life`, and stopping now costs less time than staying out or
    /// stopping on any later lap. Only under `last_lap_life` for the last lap.
    /// Where pushing ends under `flag_life` and wearing `saving_wear_fraction` of its
    /// rate would not, it stays out and drops its drifts (ADR 0086).
    Projected {
        flag_life: f32,
        last_lap_life: f32,
        saving_wear_fraction: f32,
    },
}

/// The bot constants that vary by bot level (ADR 0078): how hard a bot
/// drives, not whether it can get round, so the lookahead, the line, the
/// kerb and wheel avoidance, the pass gap and the stall recovery stay one
/// constant each. Each field with a `BOT_` constant of the same name is that
/// constant at Basic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BotProfile {
    pub pit_call: BotPitCall,
    /// Tyre life under which a bot drops its optional drifts: it still gets
    /// round a corner on full lock, slower. Zero has no floor; an Ace bot may
    /// still drop them to save its tyres, which its `pit_call` decides.
    pub drift_floor_life: f32,
    pub corner_speed_fraction: f32,
    pub rubber_band_min_factor: f32,
    pub rubber_band_max_factor: f32,
    pub item_use_delay_s: f32,
    pub start_boost_chance: f32,
}

/// The profile of each bot level, indexed by `BotLevel` in its order:
/// Basic, Pro, Ace. Basic is the named constants, so a retune of one still
/// moves Basic. The band's top is the same on every level, so the
/// slipstream's assert below holds whatever the level; Ace's bottom of one
/// is the lever: an Ace bot ahead of every human is never slowed.
pub const BOT_LEVELS: [BotProfile; 3] = [
    BotProfile {
        pit_call: BotPitCall::AtLife { life: 25.0 },
        drift_floor_life: 0.0,
        corner_speed_fraction: BOT_CORNER_SPEED_FRACTION,
        rubber_band_min_factor: BOT_RUBBER_BAND_MIN_FACTOR,
        rubber_band_max_factor: BOT_RUBBER_BAND_MAX_FACTOR,
        item_use_delay_s: BOT_ITEM_USE_DELAY_S,
        start_boost_chance: BOT_START_BOOST_CHANCE,
    },
    BotProfile {
        pit_call: BotPitCall::AtLifeOrBeforeItRunsOut { life: 30.0 },
        drift_floor_life: 30.0,
        corner_speed_fraction: 0.92,
        rubber_band_min_factor: 0.96,
        rubber_band_max_factor: BOT_RUBBER_BAND_MAX_FACTOR,
        item_use_delay_s: 1.0,
        start_boost_chance: 0.8,
    },
    BotProfile {
        pit_call: BotPitCall::Projected {
            flag_life: 15.0,
            last_lap_life: 10.0,
            saving_wear_fraction: 0.67,
        },
        drift_floor_life: 0.0,
        corner_speed_fraction: 1.0,
        rubber_band_min_factor: 1.0,
        rubber_band_max_factor: BOT_RUBBER_BAND_MAX_FACTOR,
        item_use_delay_s: 0.0,
        start_boost_chance: 1.0,
    },
];

// Camera (ADR 0020). Presentation reads these through the bridge; the sim
// never does. They live here so a feel pass tunes the camera in the same
// one-file diff as the handling it answers to.

/// Metres the chase camera sits further behind the kart at top speed than
/// at a standstill, scaling with speed in between, so speed is felt; a
/// boost past top speed pulls it back further still.
pub const CAMERA_PULL_BACK_M: f32 = 1.0;
/// Seconds the chase camera shakes after the own kart is hit.
pub const CAMERA_SHAKE_S: f32 = 0.3;
/// How far the shake throws the camera off its pose at its start; it dies
/// away over the shake.
pub const CAMERA_SHAKE_AMPLITUDE_M: f32 = 0.15;
/// Degrees the chase camera's field of view spans vertically off a boost.
pub const CAMERA_FIELD_OF_VIEW_DEG: f32 = 60.0;
/// The share the field of view widens by while a boost has more than
/// [`CAMERA_BOOST_KICK_EASE_S`] left, so a release is felt as well as seen.
pub const CAMERA_BOOST_KICK: f32 = 0.12;
/// Seconds of boost left over which the kick eases back to nothing.
pub const CAMERA_BOOST_KICK_EASE_S: f32 = 0.3;

/// Whether `seconds` is a whole number of ticks, within float rounding.
const fn is_whole_ticks(seconds: f32) -> bool {
    let ticks = seconds * TICKS_PER_SECOND as f32;
    let rounded = (ticks + 0.5) as u32 as f32;
    let error = ticks - rounded;
    error < 1e-3 && error > -1e-3
}

// Invariants between tuned values, checked at compile time so a retune that
// breaks a rule fails the build rather than a race.
const _: () = {
    assert!(MINI_TURBO_RED_CHARGE_S > MINI_TURBO_ORANGE_CHARGE_S);
    assert!(MINI_TURBO_RED_BOOST_S > MINI_TURBO_ORANGE_BOOST_S);
    // Charge thresholds are whole ticks so each tuned value names the tick
    // its tier pays on.
    assert!(is_whole_ticks(MINI_TURBO_ORANGE_CHARGE_S));
    assert!(is_whole_ticks(MINI_TURBO_RED_CHARGE_S));
    assert!(is_whole_ticks(SLIPSTREAM_CHARGE_S) && 0 < SLIPSTREAM_CHARGE_TICKS);
    assert!(0.0 < SLIPSTREAM_RANGE_M && 0.0 < SLIPSTREAM_HALF_WIDTH_M);
    assert!(0.0 < SLIPSTREAM_ANGLE_RAD && SLIPSTREAM_ANGLE_RAD < 1.0);
    assert!(0.0 < SLIPSTREAM_LEADER_SPEED_FRACTION && SLIPSTREAM_LEADER_SPEED_FRACTION < 1.0);
    assert!(0.0 < SLIPSTREAM_TOW_S && 1.0 < SLIPSTREAM_TOP_SPEED_FACTOR);
    // A drift still pays more than a tow, and a projectile still beats a
    // towed kart with a full purse under the bots' widest rubber band.
    assert!(SLIPSTREAM_TOP_SPEED_FACTOR < MINI_TURBO_BOOST_FACTOR);
    assert!(
        HOMING_SPEED_FACTOR
            > SLIPSTREAM_TOP_SPEED_FACTOR * COIN_CAP_FACTOR * BOT_RUBBER_BAND_MAX_FACTOR
    );
    assert!(BOT_RUBBER_BAND_MIN_FACTOR < 1.0 && 1.0 < BOT_RUBBER_BAND_MAX_FACTOR);
    // Every level: the band never speeds a bot ahead nor slows one behind,
    // its top is the one the slipstream's assert above covers, and each
    // level drives at least as hard as the one below it.
    let mut level = 0;
    while level < BOT_LEVELS.len() {
        let profile = BOT_LEVELS[level];
        assert!(0.0 < profile.corner_speed_fraction && profile.corner_speed_fraction <= 1.0);
        assert!(profile.rubber_band_min_factor <= 1.0 && 1.0 < profile.rubber_band_max_factor);
        assert!(profile.rubber_band_max_factor == BOT_RUBBER_BAND_MAX_FACTOR);
        assert!(0.0 <= profile.item_use_delay_s);
        assert!(0.0 <= profile.start_boost_chance && profile.start_boost_chance <= 1.0);
        if level > 0 {
            let below = BOT_LEVELS[level - 1];
            assert!(below.corner_speed_fraction <= profile.corner_speed_fraction);
            assert!(below.rubber_band_min_factor <= profile.rubber_band_min_factor);
            assert!(profile.item_use_delay_s <= below.item_use_delay_s);
            assert!(below.start_boost_chance <= profile.start_boost_chance);
        }
        level += 1;
    }
    assert!(0.0 < BOT_RUBBER_BAND_RANGE_M);
    assert!(0.0 < BOT_CORNER_SAMPLE_M && BOT_CORNER_SAMPLE_M <= BOT_LOOKAHEAD_M);
    assert!(0.0 < BOT_CORNER_SPEED_FRACTION && BOT_CORNER_SPEED_FRACTION <= 1.0);
    assert!(0.0 < BOT_HOP_STEER && BOT_HOP_STEER <= 1.0);
    assert!(BOT_AVOID_HALF_WIDTH_M > KART_HALF_WIDTH_M);
    assert!(BOT_PASS_GAP_M > 2.0 * KART_HALF_WIDTH_M);
    assert!(BOT_KERB_GAP_M > KART_HALF_WIDTH_M);
    assert!(0.0 <= BOT_WHEEL_MARGIN_S);
    assert!(0.0 < BOT_WHEEL_SET_OFF_FRACTION && BOT_WHEEL_SET_OFF_FRACTION <= 1.0);
    assert!(BOT_LOOKAHEAD_M <= BOT_KERB_APPROACH_M);
    assert!(BOT_STALL_SPEED_M_PER_S * ACCELERATION_TO_TOP_S / TOP_SPEED_M_PER_S < BOT_STALL_S);
    assert!(0.0 < START_THROTTLE_HELD && START_THROTTLE_HELD < 1.0);
    assert!(0.0 < START_WEAK_BOOST_HOLD_S && START_WEAK_BOOST_HOLD_S < START_FULL_BOOST_HOLD_S);
    assert!(START_FULL_BOOST_HOLD_S < START_STALL_HOLD_S && START_STALL_HOLD_S < COUNTDOWN_S);
    assert!(0.0 < START_WEAK_BOOST_S && START_WEAK_BOOST_S < START_BOOST_S);
    assert!(1.0 < START_WEAK_BOOST_FACTOR && START_WEAK_BOOST_FACTOR <= START_BOOST_FACTOR);
    assert!(0.0 < START_STALL_S);
    assert!(0.0 <= BOT_START_BOOST_CHANCE && BOT_START_BOOST_CHANCE <= 1.0);
    assert!(START_FULL_BOOST_HOLD_S < BOT_START_HOLD_S && BOT_START_HOLD_S < START_STALL_HOLD_S);
    assert!(0.0 < ITEM_BOX_RESPAWN_S);
    assert!(0.0 < ITEM_ROLL_S);
    assert!(0.0 < ITEM_BOX_HALF_LENGTH_M && 0.0 < ITEM_BOX_HALF_WIDTH_M);
    assert!(0.0 < ITEM_BOOST_S && 1.0 < ITEM_BOOST_FACTOR);
    assert!(0.0 < HAZARD_RADIUS_M && 0.0 < HAZARD_LIFETIME_S && 0.0 < SPIN_OUT_S);
    assert!(0.0 < HAZARD_DROP_DISTANCE_M);
    assert!(0.0 < WHEEL_WARNING_S && WHEEL_WARNING_S < WHEEL_PERIOD_S);
    assert!(0.0 < WHEEL_SPEED_M_PER_S && 0.0 < WHEEL_RADIUS_M);
    assert!(0.0 <= WHEEL_LANDING_SPREAD_M);
    assert!(WHEEL_WARNING_S <= WHEEL_FIRST_PASS_S);
    assert!(HOMING_SPEED_FACTOR > BOOST_PAD_FACTOR && HOMING_SPEED_FACTOR > ITEM_BOOST_FACTOR);
    // Nor does a full purse let a boosted kart outrun a projectile.
    assert!(
        HOMING_SPEED_FACTOR > COIN_CAP_FACTOR * BOOST_PAD_FACTOR
            && HOMING_SPEED_FACTOR > COIN_CAP_FACTOR * ITEM_BOOST_FACTOR
    );
    assert!(0.0 < HOMING_LOOKAHEAD_M && 0.0 < HOMING_LOCK_RANGE_M && 0.0 < HOMING_RADIUS_M);
    // A tick of flight never carries a projectile from outside its radius
    // of the target's box to past the box, even closing on a kart that
    // comes the other way at a full purse's top speed.
    assert!(
        (HOMING_SPEED_FACTOR + COIN_CAP_FACTOR) * TOP_SPEED_M_PER_S / (TICKS_PER_SECOND as f32)
            < 2.0 * (HOMING_RADIUS_M + KART_HALF_LENGTH_M)
    );
    assert!(0.0 < STAR_S && 1.0 < STAR_TOP_SPEED_FACTOR && 0.0 < STAR_HIT_RADIUS_M);
    assert!(0.0 < STAR_KICK_S && STAR_KICK_S < STAR_S && 1.0 < STAR_KICK_FACTOR);
    // A star kart at a full purse still cannot outrun a projectile, and
    // a tick of its travel never carries its centre over a coin.
    assert!(HOMING_SPEED_FACTOR > COIN_CAP_FACTOR * STAR_TOP_SPEED_FACTOR);
    assert!(
        COIN_CAP_FACTOR * STAR_TOP_SPEED_FACTOR * TOP_SPEED_M_PER_S / (TICKS_PER_SECOND as f32)
            < 2.0 * COIN_HALF_LENGTH_M
    );
    // A tick never carries a star kart from outside its reach of a kart's
    // box to past the box, even closing on a kart coming the other way at a
    // full purse's top speed.
    assert!(
        (COIN_CAP_FACTOR * STAR_TOP_SPEED_FACTOR + COIN_CAP_FACTOR) * TOP_SPEED_M_PER_S
            / (TICKS_PER_SECOND as f32)
            < 2.0 * (STAR_HIT_RADIUS_M + KART_HALF_LENGTH_M)
    );
    // The limiter slows a kart going forward and never touches its reverse.
    assert!(PIT_LIMIT_M_PER_S < TOP_SPEED_M_PER_S && REVERSE_TOP_SPEED_M_PER_S < PIT_LIMIT_M_PER_S);
    // The guide covers the catch at no more than twice the limit's speed, the
    // roll's chances share out all of 1, and a stop's time says its tier:
    // each tier's times start above where the last tier's end.
    assert!(PIT_BOX_CATCH_M <= PIT_LIMIT_M_PER_S * PIT_BOX_GUIDE_S * 2.0);
    assert!(0.0 < PIT_BOX_GUIDE_S);
    {
        let mut sum = 0.0;
        let mut tier = 0;
        while tier < STOP_ROLL_CHANCES.len() {
            assert!(0.0 < STOP_ROLL_CHANCES[tier]);
            sum += STOP_ROLL_CHANCES[tier];
            let [from, to] = STOP_ROLL_STATIONARY_S[tier];
            assert!(0.0 < from && from < to);
            if tier > 0 {
                assert!(STOP_ROLL_STATIONARY_S[tier - 1][1] < from);
            }
            tier += 1;
        }
        assert!(0.999_999 < sum && sum < 1.000_001);
    }
    // The blast reaches less far than its progress window, so a kart in
    // reach by distance on the same road is never dropped by the window;
    // and the window is narrower than the one a reading is searched in
    // (ADR 0046), so the blast cannot reach a road the readings themselves
    // could not tell apart.
    assert!(0.0 < HORN_RADIUS_M && HORN_RADIUS_M < HORN_PROGRESS_WINDOW_M);
    assert!(HORN_PROGRESS_WINDOW_M < PROGRESS_WINDOW_M);
    assert!(BOT_ITEM_USE_DELAY_S < BOT_HORN_HOLD_S);
    // Every band's roll weights share out the whole roll.
    {
        let mut band = 0;
        while band < ITEM_ROLL_WEIGHTS.len() {
            assert!(ITEM_ROLL_WEIGHTS[band].total() == ITEM_ROLL_PERCENT);
            band += 1;
        }
    }
    assert!(0.0 < COIN_TOP_SPEED_FACTOR && 0 < COIN_CAP);
    assert!(COINS_LOST_PER_SPIN_OUT <= COIN_CAP);
    assert!(TYRE_WEAR_BUMP_MIN_PERCENT <= TYRE_WEAR_BUMP_MAX_PERCENT);
    assert!(TYRE_WEAR_SPIN_OUT_PERCENT < TYRE_LIFE_NEW_PERCENT);
    // New tyres cost nothing, wear only ever costs more, and no factor gives
    // back more than new tyres have. A worn kart can still drift, tow a rival
    // and outrun the run-off, and its slide stays under a drift's.
    {
        let tables = [
            TYRE_TOP_SPEED_FACTORS,
            TYRE_STEER_RATE_FACTORS,
            TYRE_ACCELERATION_FACTORS,
            TYRE_BRAKING_FACTORS,
        ];
        let mut table = 0;
        while table < tables.len() {
            assert!(tables[table][0] == 1.0);
            let mut column = 1;
            while column < TYRE_LIFE_COLUMNS_PERCENT.len() {
                assert!(tables[table][column] <= tables[table][column - 1]);
                assert!(0.0 < tables[table][column] && tables[table][column] <= 1.0);
                assert!(TYRE_LIFE_COLUMNS_PERCENT[column] < TYRE_LIFE_COLUMNS_PERCENT[column - 1]);
                assert!(TYRE_WASH_OUT_MOST_RAD[column - 1] <= TYRE_WASH_OUT_MOST_RAD[column]);
                column += 1;
            }
            table += 1;
        }
    }
    assert!(TYRE_LIFE_COLUMNS_PERCENT[3] == 0.0);
    assert!(TYRE_WASH_OUT_MOST_RAD[0] == 0.0);
    assert!(TYRE_WASH_OUT_MOST_RAD[3] < DRIFT_SLIP_ANGLE_RAD);
    assert!(TYRE_WASH_OUT_LIMIT_M_PER_S2 < TYRE_WASH_OUT_FULL_M_PER_S2);
    // Full lock flat out on tyres worn right out loads them past the limit,
    // so a kart worn right out still slides; the cap keeps it under the full
    // load, so ADR 0087's bots make the lane's turn at their approach speed.
    {
        let flat_out = TOP_SPEED_M_PER_S * TYRE_TOP_SPEED_FACTORS[3];
        let full_lock_rad_per_s = (STEER_RATE_LOW_SPEED_RAD_PER_S
            - (STEER_RATE_LOW_SPEED_RAD_PER_S - STEER_RATE_TOP_SPEED_RAD_PER_S)
                * TYRE_TOP_SPEED_FACTORS[3])
            * TYRE_STEER_RATE_FACTORS[3];
        assert!(TYRE_WASH_OUT_LIMIT_M_PER_S2 < flat_out * full_lock_rad_per_s);
        assert!(flat_out * full_lock_rad_per_s < TYRE_WASH_OUT_FULL_M_PER_S2);
    }
    assert!(0.0 < TYRE_WASH_OUT_RATE_RAD_PER_S);
    // A hop's airtime lets the deepest slide go, so a kart lands on its line.
    assert!(TYRE_WASH_OUT_MOST_RAD[3] <= TYRE_WASH_OUT_RATE_RAD_PER_S * HOP_AIRTIME_S);
    // The lap costs run from new tyres, costing nothing, down to worn out,
    // costing the most, so a life between two rows reads between them.
    {
        assert!(BOT_TYRE_LOSS_S_PER_LAP[0][0] == TYRE_LIFE_NEW_PERCENT);
        assert!(BOT_TYRE_LOSS_S_PER_LAP[0][1] == 0.0);
        let mut row = 1;
        while row < BOT_TYRE_LOSS_S_PER_LAP.len() {
            assert!(BOT_TYRE_LOSS_S_PER_LAP[row][0] < BOT_TYRE_LOSS_S_PER_LAP[row - 1][0]);
            assert!(BOT_TYRE_LOSS_S_PER_LAP[row][1] >= BOT_TYRE_LOSS_S_PER_LAP[row - 1][1]);
            row += 1;
        }
        assert!(BOT_TYRE_LOSS_S_PER_LAP[BOT_TYRE_LOSS_S_PER_LAP.len() - 1][0] == 0.0);
    }
    assert!(DRIFT_ENTRY_SPEED_FRACTION < TYRE_TOP_SPEED_FACTORS[3]);
    assert!(SLIPSTREAM_LEADER_SPEED_FRACTION < TYRE_TOP_SPEED_FACTORS[3]);
    assert!(OFF_TRACK_SPEED_FACTOR < TYRE_TOP_SPEED_FACTORS[3]);
    // The wire carries the count in a byte.
    assert!(COIN_CAP <= u8::MAX as u32);
    assert!(0.0 < COIN_HALF_LENGTH_M && 0.0 < COIN_HALF_WIDTH_M && 0.0 < COIN_RESPAWN_S);
    // A tick at a full purse's boosted speed never carries a kart's centre
    // over a coin without landing on it.
    assert!(
        COIN_CAP_FACTOR * BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S / (TICKS_PER_SECOND as f32)
            < 2.0 * COIN_HALF_LENGTH_M
    );
    assert!(
        HOMING_SPEED_FACTOR * TOP_SPEED_M_PER_S / (TICKS_PER_SECOND as f32) < PROGRESS_WINDOW_M
    );
    assert!(BOT_KERB_APPROACH_M + KERB_HALF_LENGTH_M < PROGRESS_WINDOW_M);
    assert!(0.0 < BOT_ITEM_USE_DELAY_S);
    assert!(0.0 < BOT_HAZARD_HOLD_S && BOT_HAZARD_HOLD_S <= BOT_ITEM_USE_DELAY_S);
    assert!(0.0 < PREDICTED_FIRE_TIMEOUT_S);
    assert!(0.0 < RACE_END_CAP_S && 0.0 < RACE_OUTRO_S);
    assert!(0.0 <= CAMERA_PULL_BACK_M);
    assert!(0.0 < CAMERA_SHAKE_S && 0.0 <= CAMERA_SHAKE_AMPLITUDE_M);
    // The shake is over before the spin-out it answers to ends.
    assert!(CAMERA_SHAKE_S < SPIN_OUT_S);
    assert!(0.0 < CAMERA_FIELD_OF_VIEW_DEG && 0.0 <= CAMERA_BOOST_KICK);
    // Every boost, the shortest included, reaches the full kick before it
    // eases back.
    assert!(0.0 < CAMERA_BOOST_KICK_EASE_S && CAMERA_BOOST_KICK_EASE_S < MINI_TURBO_ORANGE_BOOST_S);
    assert!(SNAPSHOTS_PER_SECOND <= TICKS_PER_SECOND);
    assert!(0 < SNAPSHOT_EVENT_REPEATS);
    assert!(0 < INPUT_QUEUE_START_TICKS && INPUT_QUEUE_START_TICKS as u32 <= INPUT_QUEUE_MAX_TICKS);
    assert!(0.0 < INTERPOLATION_DELAY_S);
    assert!(0.0 < INTERPOLATION_CLOCK_GAIN && 0.0 < INTERPOLATION_CLOCK_SLEW);
    assert!(INTERPOLATION_CLOCK_SLEW < 1.0);
    assert!(INTERPOLATION_DELAY_S < INTERPOLATION_RESYNC_S);
    assert!(0 < RECONCILIATION_BLEND_TICKS);
    assert!(0.0 < RECONCILIATION_TOLERANCE_M);
    assert!(RECONCILIATION_TOLERANCE_M < RECONCILIATION_VISIBLE_CORRECTION_M);
    assert!(RELIABLE_FRAME_MAX_BYTES > 1024);
    assert!(ROOM_PLAYERS_MAX <= KARTS_PER_RACE);
    assert!(0 < NICKNAME_MIN_CHARS && NICKNAME_MIN_CHARS <= NICKNAME_MAX_CHARS);
    assert!(0.0 < ROOM_CLOSE_AFTER_EMPTY_S);
    assert!(0 < ROOMS_MAX);
    assert!(0 < ROOM_CREATES_PER_ADDRESS_MAX);
    assert!(0.0 < ROOM_CREATE_WINDOW_S);
    // One address alone cannot fill the server with rooms it leaves empty.
    assert!(
        (ROOM_CREATES_PER_ADDRESS_MAX as f32 * ROOM_CLOSE_AFTER_EMPTY_S / ROOM_CREATE_WINDOW_S)
            < ROOMS_MAX as f32 / 2.0
    );
    assert!(TICKS_PER_SECOND.is_multiple_of(SNAPSHOTS_PER_SECOND));
    assert!(0.0 < DRIFT_ENTRY_SPEED_FRACTION && DRIFT_ENTRY_SPEED_FRACTION < 1.0);
    assert!(0.0 <= DRIFT_TURN_STEER_RANGE && DRIFT_TURN_STEER_RANGE < DRIFT_TURN_NEUTRAL);
    assert!(0.0 <= DRIFT_TURN_BRAKE);
    assert!(0.0 < OFF_TRACK_SPEED_FACTOR && OFF_TRACK_SPEED_FACTOR < 1.0);
    assert!(BRAKE_TO_STOP_S < COAST_TO_STOP_S);
    assert!(0.0 < REVERSE_TOP_SPEED_M_PER_S && REVERSE_TOP_SPEED_M_PER_S < TOP_SPEED_M_PER_S);
    assert!(0.0 < KART_EDGE_RADIUS_M);
    assert!(KART_EDGE_RADIUS_M < KART_HALF_LENGTH_M);
    assert!(KART_EDGE_RADIUS_M < KART_HALF_WIDTH_M);
    assert!(KART_EDGE_RADIUS_M < KART_HALF_HEIGHT_M);
};

/// What the browser reads of these values, gathered so the bridge hands
/// them over in one call, decoded into the generated TypeScript type
/// (ADR 0026). Each field is the constant of the same name.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, ts_rs::TS)]
#[ts(export_to = "../sim/")]
#[serde(rename_all = "camelCase")]
pub struct Tuning {
    pub ticks_per_second: u32,
    pub lap_choices: [u32; 2],
    pub karts_per_race: usize,
    /// The kart's width, height and length, so the View draws the box the
    /// sim collides with.
    pub kart_size_m: [f32; 3],
    pub replay_hash_interval_ticks: u32,
    pub countdown_s: f32,
    pub predicted_fire_timeout_s: f32,
    pub item_box_respawn_s: f32,
    /// How long a box's item rolls in the slot, which the reel runs over.
    pub item_roll_s: f32,
    /// Indexed by the coins a kart carries.
    pub coin_top_speeds_m_per_s: [f32; COIN_CAP as usize + 1],
    pub coin_cap: u32,
    pub tyre_life_new_percent: f32,
    /// The life from which a kart shows wear and the BOX call can sound.
    pub tyre_life_cliff_percent: f32,
    /// The longest a clean stop lasts: past it the crew shows what went wrong (ADR 0082).
    pub stop_clean_s: f32,
    /// How long the guide onto the mark takes: the pit crew runs out at its start.
    pub pit_box_guide_s: f32,
    pub coins_lost_per_spin_out: u32,
    pub coin_half_length_m: f32,
    pub coin_half_width_m: f32,
    pub coin_respawn_s: f32,
    pub hazard_drop_distance_m: f32,
    pub spin_out_s: f32,
    /// How long a star's run lasts, which the Instrument panel's ring counts
    /// down and the star music is joined against.
    pub star_s: f32,
    /// How long a wake takes to charge, which the wind lines build over.
    pub slipstream_charge_s: f32,
    /// How far the Air Horn's blast reaches, so the View's ring grows to
    /// the reach.
    pub horn_radius_m: f32,
    /// The runaway wheel's reach, so the View draws a wheel that size.
    pub wheel_radius_m: f32,
    pub top_speed_m_per_s: f32,
    pub camera_pull_back_m: f32,
    pub camera_shake_s: f32,
    pub camera_shake_amplitude_m: f32,
    pub camera_field_of_view_deg: f32,
    pub camera_boost_kick: f32,
    pub camera_boost_kick_ease_s: f32,
    pub webtransport_connect_timeout_s: f32,
    pub ping_timeout_s: f32,
    pub ping_resend_s: f32,
    pub connection_silence_s: f32,
    pub reconnect_retry_s: f32,
    pub nickname_min_chars: usize,
    pub nickname_max_chars: usize,
    pub room_code_length: usize,
}

pub const TUNING: Tuning = Tuning {
    ticks_per_second: TICKS_PER_SECOND,
    lap_choices: LAP_CHOICES,
    karts_per_race: KARTS_PER_RACE,
    kart_size_m: [
        2.0 * KART_HALF_WIDTH_M,
        2.0 * KART_HALF_HEIGHT_M,
        2.0 * KART_HALF_LENGTH_M,
    ],
    replay_hash_interval_ticks: REPLAY_HASH_INTERVAL_TICKS,
    countdown_s: COUNTDOWN_S,
    predicted_fire_timeout_s: PREDICTED_FIRE_TIMEOUT_S,
    item_box_respawn_s: ITEM_BOX_RESPAWN_S,
    item_roll_s: ITEM_ROLL_S,
    coin_top_speeds_m_per_s: COIN_TOP_SPEEDS_M_PER_S,
    coin_cap: COIN_CAP,
    tyre_life_new_percent: TYRE_LIFE_NEW_PERCENT,
    tyre_life_cliff_percent: TYRE_LIFE_CLIFF_PERCENT,
    stop_clean_s: STOP_ROLL_STATIONARY_S[1][1],
    pit_box_guide_s: PIT_BOX_GUIDE_S,
    coins_lost_per_spin_out: COINS_LOST_PER_SPIN_OUT,
    coin_half_length_m: COIN_HALF_LENGTH_M,
    coin_half_width_m: COIN_HALF_WIDTH_M,
    coin_respawn_s: COIN_RESPAWN_S,
    hazard_drop_distance_m: HAZARD_DROP_DISTANCE_M,
    spin_out_s: SPIN_OUT_S,
    star_s: STAR_S,
    slipstream_charge_s: SLIPSTREAM_CHARGE_S,
    horn_radius_m: HORN_RADIUS_M,
    wheel_radius_m: WHEEL_RADIUS_M,
    top_speed_m_per_s: TOP_SPEED_M_PER_S,
    camera_pull_back_m: CAMERA_PULL_BACK_M,
    camera_shake_s: CAMERA_SHAKE_S,
    camera_shake_amplitude_m: CAMERA_SHAKE_AMPLITUDE_M,
    camera_field_of_view_deg: CAMERA_FIELD_OF_VIEW_DEG,
    camera_boost_kick: CAMERA_BOOST_KICK,
    camera_boost_kick_ease_s: CAMERA_BOOST_KICK_EASE_S,
    webtransport_connect_timeout_s: WEBTRANSPORT_CONNECT_TIMEOUT_S,
    ping_timeout_s: PING_TIMEOUT_S,
    ping_resend_s: PING_RESEND_S,
    connection_silence_s: CONNECTION_SILENCE_S,
    reconnect_retry_s: RECONNECT_RETRY_S,
    nickname_min_chars: NICKNAME_MIN_CHARS,
    nickname_max_chars: NICKNAME_MAX_CHARS,
    room_code_length: ROOM_CODE_LENGTH,
};
