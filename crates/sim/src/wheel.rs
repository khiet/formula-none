//! The runaway wheel (GLOSSARY, ADR 0042): a pure function of the race
//! clock and the race seed, so the server, the prediction and a replay all
//! see the same pass at the same moment and no snapshot carries it. Every
//! crossing runs on the one cycle: passes are numbered from 1, the first
//! starting at `WHEEL_FIRST_PASS_S`, odd passes rolling in from the
//! crossing's left wall and even ones from its right. A pass is a path of
//! straight legs, wall to wall, at a constant speed: it rolls in on the
//! crossing's line, rebounds `WHEEL_BOUNCES` times where its tyre meets a
//! wall, and leaves through the far wall. Each leg ends at a point drawn
//! from the seed within `WHEEL_LANDING_SPREAD_M` of the crossing's line.

use crate::track::WheelCrossing;
use crate::tuning::{
    WHEEL_BOUNCES, WHEEL_FIRST_PASS_S, WHEEL_LANDING_SPREAD_M, WHEEL_PERIOD_S, WHEEL_RADIUS_M,
    WHEEL_SPEED_M_PER_S, WHEEL_WARNING_S,
};
use crate::Track;

/// The legs of a pass: one more than its rebounds.
const LEGS: usize = WHEEL_BOUNCES as usize + 1;

/// The wheel at a crossing at one moment of the race clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wheel {
    /// Off the course, the flag down.
    Quiet,
    /// The flag is up for the pass about to start.
    Warning {
        /// The pass about to start, from 1.
        pass: u32,
        /// Where it rolls in from: the foot of the wall it starts at, on
        /// the crossing's line, on the surface, in metres.
        from: [f32; 3],
        /// The direction it will roll in, as `Rolling`'s yaw.
        yaw: f32,
    },
    /// Rolling over the track, the flag still up.
    Rolling {
        /// The pass under way, from 1.
        pass: u32,
        /// The wheel's centre, on the surface, in metres.
        position: [f32; 3],
        /// The direction it rolls in, in radians about +Y, zero facing -Z;
        /// it turns at each wall it rebounds from.
        yaw: f32,
        /// How far along the pass's path it is, from 0 at the wall it
        /// rolled in from to 1 at the wall it leaves through.
        fraction: f32,
        /// How far through the leg under way it is, from 0 at the wall it
        /// last left to 1 at the next.
        leg_fraction: f32,
    },
}

/// The wheel at the track's crossing `crossing` with the race clock at
/// `elapsed_s`, in a race from `seed`. Only a racing clock rolls it;
/// through the countdown and the outro it is quiet, which the caller says
/// by not asking.
pub fn wheel(track: &Track, crossing: usize, seed: u64, elapsed_s: f32) -> Wheel {
    let crossing = &track.wheel_crossings()[crossing];
    let since_flag_s = elapsed_s - (WHEEL_FIRST_PASS_S - WHEEL_WARNING_S);
    if since_flag_s < 0.0 {
        return Wheel::Quiet;
    }
    let cycles = libm::floorf(since_flag_s / WHEEL_PERIOD_S);
    let into_cycle_s = since_flag_s - cycles * WHEEL_PERIOD_S;
    let pass = cycles as u32 + 1;
    let path = Path::new(crossing, seed, pass);
    if into_cycle_s < WHEEL_WARNING_S {
        let (from, yaw) = path.place(crossing, 0, 0.0);
        return Wheel::Warning { pass, from, yaw };
    }
    let rolled_m = (into_cycle_s - WHEEL_WARNING_S) * WHEEL_SPEED_M_PER_S;
    if rolled_m > path.length_m() {
        return Wheel::Quiet;
    }
    let (leg, leg_fraction) = path.leg_at(rolled_m);
    let (position, yaw) = path.place(crossing, leg, leg_fraction);
    Wheel::Rolling {
        pass,
        position,
        yaw,
        fraction: rolled_m / path.length_m(),
        leg_fraction,
    }
}

/// The longest a pass across a crossing `half_width` either side of its
/// centre can last: every landing at the far end of its spread from the
/// last.
pub(crate) fn longest_pass_s(half_width: f32) -> f32 {
    let rebound = half_width - WHEEL_RADIUS_M;
    let mut length_m = 0.0;
    for leg in 0..LEGS {
        let from = if leg == 0 { half_width } else { rebound };
        let to = if leg + 1 == LEGS { half_width } else { rebound };
        let along = if leg == 0 { 1.0 } else { 2.0 } * WHEEL_LANDING_SPREAD_M;
        length_m += libm::sqrtf((from + to) * (from + to) + along * along);
    }
    length_m / WHEEL_SPEED_M_PER_S
}

/// One pass's path over a crossing, from the seed: points in metres right
/// of the crossing's centre and ahead of it along the track, the first on
/// the wall it rolls in from and the last on the wall it leaves through.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Path {
    points: [[f32; 2]; LEGS + 1],
    lengths_m: [f32; LEGS],
}

impl Path {
    pub(crate) fn new(crossing: &WheelCrossing, seed: u64, pass: u32) -> Self {
        let half = crossing.half_width;
        // Odd passes roll in from the left wall, toward the right.
        let side = if pass % 2 == 1 { 1.0 } else { -1.0 };
        let mut points = [[-side * half, 0.0]; LEGS + 1];
        for leg in 0..LEGS {
            // The wheel rebounds as its tyre meets the wall and leaves
            // through the far wall whole.
            let wall = if leg % 2 == 0 { side } else { -side };
            let across = if leg + 1 == LEGS {
                half
            } else {
                half - WHEEL_RADIUS_M
            };
            let landing = (2.0 * unit(seed, pass, leg as u32) - 1.0) * WHEEL_LANDING_SPREAD_M;
            points[leg + 1] = [wall * across, landing];
        }
        let mut lengths_m = [0.0; LEGS];
        for (leg, length) in lengths_m.iter_mut().enumerate() {
            let [from, to] = [points[leg], points[leg + 1]];
            let (right, ahead) = (to[0] - from[0], to[1] - from[1]);
            *length = libm::sqrtf(right * right + ahead * ahead);
        }
        Self { points, lengths_m }
    }

    pub(crate) fn length_m(&self) -> f32 {
        self.lengths_m.iter().sum()
    }

    /// How long the wheel takes to roll the whole path.
    pub(crate) fn lasts_s(&self) -> f32 {
        self.length_m() / WHEEL_SPEED_M_PER_S
    }

    /// The legs in order, each as where it starts, where it ends, the
    /// seconds into the pass it starts at and the seconds it lasts.
    pub(crate) fn legs(&self) -> impl Iterator<Item = ([f32; 2], [f32; 2], f32, f32)> + '_ {
        let mut starts_s = 0.0;
        (0..LEGS).map(move |leg| {
            let lasts_s = self.lengths_m[leg] / WHEEL_SPEED_M_PER_S;
            let leg_starts_s = starts_s;
            starts_s += lasts_s;
            (
                self.points[leg],
                self.points[leg + 1],
                leg_starts_s,
                lasts_s,
            )
        })
    }

    /// The leg the wheel is on `rolled_m` along the path, and how far
    /// through it; past the end, the end of the last.
    fn leg_at(&self, rolled_m: f32) -> (usize, f32) {
        let mut left_m = rolled_m;
        for (leg, &length) in self.lengths_m.iter().enumerate() {
            if left_m <= length || leg + 1 == LEGS {
                return (leg, (left_m / length).min(1.0));
            }
            left_m -= length;
        }
        unreachable!("a path has legs")
    }

    fn point(&self, leg: usize, fraction: f32) -> [f32; 2] {
        let [from, to] = [self.points[leg], self.points[leg + 1]];
        [
            from[0] + (to[0] - from[0]) * fraction,
            from[1] + (to[1] - from[1]) * fraction,
        ]
    }

    /// Where on `crossing` the wheel is `fraction` through leg `leg`, on
    /// the surface, and the direction it rolls in there.
    fn place(&self, crossing: &WheelCrossing, leg: usize, fraction: f32) -> ([f32; 3], f32) {
        let [right, ahead] = self.point(leg, fraction);
        let [to_right, to_ahead] = self.heading(leg);
        // Forward of the crossing's yaw is (-sin, -cos) in x, z and right of
        // it (cos, -sin); yaw grows counter-clockwise, so a heading to the
        // right turns it clockwise.
        let (sin, cos) = libm::sincosf(crossing.yaw);
        let [x, y, z] = crossing.position;
        (
            [
                x + right * cos - ahead * sin,
                y,
                z - right * sin - ahead * cos,
            ],
            crossing.yaw + libm::atan2f(-to_right, to_ahead),
        )
    }

    fn heading(&self, leg: usize) -> [f32; 2] {
        let [from, to] = [self.points[leg], self.points[leg + 1]];
        [to[0] - from[0], to[1] - from[1]]
    }
}

/// When pass `pass` starts rolling, on the race clock.
pub(crate) fn pass_start_s(pass: u32) -> f32 {
    WHEEL_FIRST_PASS_S + (pass - 1) as f32 * WHEEL_PERIOD_S
}

/// The first pass over `crossing` that has not ended by `elapsed_s` on the
/// race clock: the one rolling then, or else the next.
pub(crate) fn pass_not_over(crossing: &WheelCrossing, seed: u64, elapsed_s: f32) -> u32 {
    let since_first_s = elapsed_s - WHEEL_FIRST_PASS_S;
    if since_first_s <= 0.0 {
        return 1;
    }
    let pass = libm::floorf(since_first_s / WHEEL_PERIOD_S) as u32 + 1;
    let path = Path::new(crossing, seed, pass);
    let ends_s = pass_start_s(pass) + path.lasts_s();
    if elapsed_s < ends_s {
        pass
    } else {
        pass + 1
    }
}

/// A number in [0, 1) that only `seed`, `pass` and `leg` decide: an
/// integer hash, so it is the same natively and in wasm and draws nothing
/// from the race's item generator.
fn unit(seed: u64, pass: u32, leg: u32) -> f32 {
    let mut hash =
        seed ^ (u64::from(pass) << 32 | u64::from(leg)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // The SplitMix64 finaliser.
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 31;
    // The top 24 bits, which an f32 holds exactly.
    (hash >> 40) as f32 / (1u32 << 24) as f32
}
