//! Single home for gameplay, item, bot and network constants (ADR 0014).
//! Tuning a value means editing this file and nothing else; no other module
//! may hard-code a number that belongs here. Initial values are the PRD's.

// Rates. Every rule is written per tick; frame rate never enters the sim.

/// Fixed sim rate on both client and server.
pub const TICKS_PER_SECOND: u32 = 60;
pub const SNAPSHOTS_PER_SECOND: u32 = 30;
/// How far behind the newest snapshot other karts are rendered.
pub const INTERPOLATION_DELAY_S: f32 = 0.1;
/// Upper bound on the frames a reconciliation correction is blended over.
pub const RECONCILIATION_BLEND_FRAMES: u32 = 6;
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

// Race.

/// Karts on the grid; empty slots are filled by bots at Start.
pub const KARTS_PER_RACE: usize = 8;
pub const LAPS_PER_RACE: u32 = 3;
/// Seconds after the first finisher before the race ends and unfinished
/// humans are handed to bots.
pub const RACE_END_GRACE_S: f32 = 20.0;
/// Seconds of decreasing spline progress before the wrong-way warning.
pub const WRONG_WAY_WARNING_S: f32 = 2.0;

// World. Metres and seconds. The kart's collider is a box of these
// dimensions; the View draws the same box until the glTF kart lands (ADR 0007).

pub const GRAVITY_M_PER_S2: f32 = -9.81;
/// Half the kart's length, nose to tail along its facing.
pub const KART_HALF_LENGTH_M: f32 = 0.9;
/// Half the kart's width, wheel to wheel.
pub const KART_HALF_WIDTH_M: f32 = 0.6;
/// Half the kart's height; a kart at rest sits this far above the surface.
pub const KART_HALF_HEIGHT_M: f32 = 0.3;
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
/// Radians between where a drifting kart points and where it travels; the
/// nose leads into the corner by this much.
pub const DRIFT_SLIP_ANGLE_RAD: f32 = 0.3;
pub const MINI_TURBO_BLUE_CHARGE_S: f32 = 0.8;
pub const MINI_TURBO_BLUE_BOOST_S: f32 = 0.6;
pub const MINI_TURBO_ORANGE_CHARGE_S: f32 = 1.6;
pub const MINI_TURBO_ORANGE_BOOST_S: f32 = 1.0;
pub const MINI_TURBO_BOOST_FACTOR: f32 = 1.3;
pub const BOOST_PAD_S: f32 = 1.0;
pub const BOOST_PAD_FACTOR: f32 = 1.4;
/// The pad is a rectangle on the surface, this big either side of its
/// placement along and across the direction it faces; a kart whose centre
/// is on it while on the ground is boosted. Half the asphalt wide, so a
/// pad can be missed on purpose.
pub const BOOST_PAD_HALF_LENGTH_M: f32 = 2.0;
pub const BOOST_PAD_HALF_WIDTH_M: f32 = 3.0;
/// Pressing accelerate inside this window before Go gives a start boost;
/// earlier stalls the kart for `START_STALL_S`.
pub const START_BOOST_WINDOW_S: f32 = 0.3;
pub const START_BOOST_S: f32 = 1.0;
pub const START_BOOST_FACTOR: f32 = 1.3;
pub const START_STALL_S: f32 = 1.0;
/// Top speed fraction on sand and grass.
pub const OFF_TRACK_SPEED_FACTOR: f32 = 0.55;
/// Seconds outside the world bounds before respawn at the last checkpoint.
pub const RESPAWN_DELAY_S: f32 = 1.5;

// Items.

pub const ITEM_BOX_RESPAWN_S: f32 = 3.0;
pub const ITEM_BOOST_S: f32 = 1.5;
pub const ITEM_BOOST_FACTOR: f32 = 1.4;
/// Metres behind the kart a hazard is dropped.
pub const HAZARD_DROP_DISTANCE_M: f32 = 1.0;
pub const HAZARD_LIFETIME_S: f32 = 30.0;
pub const SPIN_OUT_S: f32 = 1.0;
/// Homing projectile speed as a multiple of top speed.
pub const HOMING_SPEED_FACTOR: f32 = 1.6;

/// Item roll weights for one band of race positions, in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemRollWeights {
    /// First and last race position (1-based, inclusive) this band covers.
    pub positions: (u8, u8),
    pub hazard: u8,
    pub boost: u8,
    pub homing: u8,
}

/// Roll weights by race position. Bands are contiguous and cover every grid
/// slot; each band's weights sum to 100 so a roll in 0..100 picks one item.
pub const ITEM_ROLL_WEIGHTS: [ItemRollWeights; 3] = [
    ItemRollWeights {
        positions: (1, 2),
        hazard: 70,
        boost: 20,
        homing: 10,
    },
    ItemRollWeights {
        positions: (3, 5),
        hazard: 30,
        boost: 40,
        homing: 30,
    },
    ItemRollWeights {
        positions: (6, 8),
        hazard: 10,
        boost: 40,
        homing: 50,
    },
];

// Bots.

/// Metres ahead on the centreline spline a bot steers toward.
pub const BOT_LOOKAHEAD_M: f32 = 12.0;
/// Seconds after pickup within which a bot uses an item if a target exists.
pub const BOT_ITEM_USE_DELAY_S: f32 = 2.0;
/// Top speed scale by distance to the nearest human: slower far ahead,
/// faster far behind.
pub const BOT_RUBBER_BAND_MIN_FACTOR: f32 = 0.92;
pub const BOT_RUBBER_BAND_MAX_FACTOR: f32 = 1.08;

// Invariants between tuned values, checked at compile time so a retune that
// breaks a rule fails the build rather than a race.
const _: () = {
    assert!(MINI_TURBO_ORANGE_CHARGE_S > MINI_TURBO_BLUE_CHARGE_S);
    assert!(MINI_TURBO_ORANGE_BOOST_S > MINI_TURBO_BLUE_BOOST_S);
    assert!(BOT_RUBBER_BAND_MIN_FACTOR < 1.0 && 1.0 < BOT_RUBBER_BAND_MAX_FACTOR);
    assert!(SNAPSHOTS_PER_SECOND <= TICKS_PER_SECOND);
    assert!(TICKS_PER_SECOND.is_multiple_of(SNAPSHOTS_PER_SECOND));
    assert!(0.0 < DRIFT_ENTRY_SPEED_FRACTION && DRIFT_ENTRY_SPEED_FRACTION < 1.0);
    assert!(0.0 <= DRIFT_TURN_STEER_RANGE && DRIFT_TURN_STEER_RANGE < DRIFT_TURN_NEUTRAL);
    assert!(0.0 < OFF_TRACK_SPEED_FACTOR && OFF_TRACK_SPEED_FACTOR < 1.0);
    assert!(BRAKE_TO_STOP_S < COAST_TO_STOP_S);
    assert!(0.0 < REVERSE_TOP_SPEED_M_PER_S && REVERSE_TOP_SPEED_M_PER_S < TOP_SPEED_M_PER_S);
    assert!(0.0 < KART_EDGE_RADIUS_M);
    assert!(KART_EDGE_RADIUS_M < KART_HALF_LENGTH_M);
    assert!(KART_EDGE_RADIUS_M < KART_HALF_WIDTH_M);
    assert!(KART_EDGE_RADIUS_M < KART_HALF_HEIGHT_M);
};
