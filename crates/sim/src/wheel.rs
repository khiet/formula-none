//! The runaway wheel (GLOSSARY, ADR 0041): a pure function of the race
//! clock, so the server, the prediction and a replay all see the same pass
//! at the same moment and no snapshot carries it. Every crossing runs on
//! the one cycle: passes are numbered from 1, the first starting at
//! `WHEEL_FIRST_PASS_S`, odd passes rolling from the crossing's left wall
//! to its right and even ones back.

use crate::track::WheelCrossing;
use crate::tuning::{WHEEL_FIRST_PASS_S, WHEEL_PASS_S, WHEEL_PERIOD_S, WHEEL_WARNING_S};
use crate::Track;

/// The wheel at a crossing at one moment of the race clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wheel {
    /// Off the course, the flag down.
    Quiet,
    /// The flag is up for the pass about to start.
    Warning,
    /// Crossing the track, the flag still up.
    Rolling {
        /// The pass under way, from 1.
        pass: u32,
        /// The wheel's centre, on the surface, in metres.
        position: [f32; 3],
        /// The direction it rolls in, in radians about +Y, zero facing -Z.
        yaw: f32,
        /// How far through the pass it is, from 0 at one wall to 1 at the
        /// other.
        fraction: f32,
    },
}

/// The wheel at the track's crossing `crossing` with the race clock at
/// `elapsed_s`. Only a racing clock rolls it; through the countdown and
/// the outro it is quiet, which the caller says by not asking.
pub fn wheel(track: &Track, crossing: usize, elapsed_s: f32) -> Wheel {
    let crossing = &track.wheel_crossings()[crossing];
    let since_flag_s = elapsed_s - (WHEEL_FIRST_PASS_S - WHEEL_WARNING_S);
    if since_flag_s < 0.0 {
        return Wheel::Quiet;
    }
    let cycles = libm::floorf(since_flag_s / WHEEL_PERIOD_S);
    let into_cycle_s = since_flag_s - cycles * WHEEL_PERIOD_S;
    if into_cycle_s < WHEEL_WARNING_S {
        return Wheel::Warning;
    }
    let fraction = (into_cycle_s - WHEEL_WARNING_S) / WHEEL_PASS_S;
    if fraction > 1.0 {
        return Wheel::Quiet;
    }
    let pass = cycles as u32 + 1;
    let rightward = rolls_rightward(pass);
    let right = across(crossing, rightward, fraction);
    // Right of the crossing's yaw, seen from above, is (cos, -sin) in x, z;
    // yaw grows counter-clockwise, so rolling right is a quarter turn less.
    let (sin, cos) = libm::sincosf(crossing.yaw);
    let [x, y, z] = crossing.position;
    let quarter = std::f32::consts::FRAC_PI_2;
    Wheel::Rolling {
        pass,
        position: [x + right * cos, y, z - right * sin],
        yaw: if rightward {
            crossing.yaw - quarter
        } else {
            crossing.yaw + quarter
        },
        fraction,
    }
}

/// When pass `pass` reaches `right` metres right of the crossing's centre,
/// on the race clock; `right` is clamped to the crossing.
pub(crate) fn reaches_across_s(crossing: &WheelCrossing, pass: u32, right: f32) -> f32 {
    let half = crossing.half_width;
    let from_left = (right.clamp(-half, half) + half) / (2.0 * half);
    let fraction = if rolls_rightward(pass) {
        from_left
    } else {
        1.0 - from_left
    };
    pass_start_s(pass) + fraction * WHEEL_PASS_S
}

/// The first pass that has not ended by `elapsed_s` on the race clock:
/// the one rolling then, or else the next.
pub(crate) fn pass_not_over(elapsed_s: f32) -> u32 {
    let since_first_end_s = elapsed_s - (WHEEL_FIRST_PASS_S + WHEEL_PASS_S);
    if since_first_end_s <= 0.0 {
        return 1;
    }
    libm::ceilf(since_first_end_s / WHEEL_PERIOD_S) as u32 + 1
}

fn pass_start_s(pass: u32) -> f32 {
    WHEEL_FIRST_PASS_S + (pass - 1) as f32 * WHEEL_PERIOD_S
}

fn rolls_rightward(pass: u32) -> bool {
    pass % 2 == 1
}

/// Metres right of the crossing's centre a pass rolling `rightward` is at
/// `fraction` of the way through.
fn across(crossing: &WheelCrossing, rightward: bool, fraction: f32) -> f32 {
    let half = crossing.half_width;
    let from_left = -half + fraction * 2.0 * half;
    if rightward {
        from_left
    } else {
        -from_left
    }
}
