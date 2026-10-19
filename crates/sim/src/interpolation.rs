//! The other karts as the client draws them (ADR 0009): between the two
//! snapshots it has received either side of a render clock that runs the
//! tuned delay behind the newest. The delay holds several snapshots, so one
//! lost or late leaves a pair to draw between and the karts move on as
//! smoothly as before.
//!
//! The render clock runs one tick per client tick, nudged toward the delay
//! behind the newest snapshot by a share of how far it is off, so it follows
//! the server's rate and not each snapshot's jitter. Between snapshots the
//! newest counts as the ticks since it arrived further on, up to the next
//! one's due, so the clock is not pulled back and forth at the snapshot
//! rate, and a lost snapshot holds it back only a little. It never runs past the newest
//! snapshot: what the client has not received is never read, and a kart
//! starved of snapshots waits at the newest rather than guessing on.
//!
//! A kart that moved further between two snapshots than any kart can drive
//! was put there, by a respawn; it is drawn where it was until the snapshot
//! that has it in its new place, never streaking across the track between.

use std::collections::VecDeque;

use rapier3d::prelude::*;

use crate::tuning::{
    BOOST_PAD_FACTOR, INTERPOLATION_CLOCK_GAIN, INTERPOLATION_CLOCK_SLEW, INTERPOLATION_DELAY_S,
    INTERPOLATION_RESYNC_S, SNAPSHOTS_PER_SECOND, TICKS_PER_SECOND, TICK_S, TOP_SPEED_M_PER_S,
};
use crate::{KartTransform, SnapshotKart};

const DELAY_TICKS: f32 = INTERPOLATION_DELAY_S * TICKS_PER_SECOND as f32;
const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;
const RESYNC_TICKS: f32 = INTERPOLATION_RESYNC_S * TICKS_PER_SECOND as f32;
/// Further than any kart drives in a tick: twice the biggest boost's top
/// speed, so a kart thrown by a contact is still driving.
const FARTHEST_DRIVE_PER_TICK_M: f32 = 2.0 * BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S * TICK_S;

/// Every kart of a room's race as the snapshots have it, drawn the tuned
/// delay behind the newest.
#[derive(Default)]
pub struct Interpolation {
    /// The snapshots received, in tick order, from the last at or before the
    /// render clock on.
    snapshots: VecDeque<(u32, Vec<SnapshotKart>)>,
    /// The server tick the karts are drawn at; `None` before a snapshot.
    render_tick: Option<f32>,
    /// Client ticks advanced since the newest snapshot arrived.
    since_newest: u32,
}

impl Interpolation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold the snapshot of `tick`, which said `karts` of every kart in grid
    /// order. One the render clock has passed, or already held, changes
    /// nothing; one that arrives after a newer one fills its place.
    pub fn push(&mut self, tick: u32, karts: Vec<SnapshotKart>) {
        let passed = self
            .render_tick
            .is_some_and(|render| (tick as f32) < render)
            && self
                .snapshots
                .front()
                .is_some_and(|&(first, _)| tick < first);
        if passed {
            return;
        }
        let at = self.snapshots.partition_point(|&(held, _)| held < tick);
        if self
            .snapshots
            .get(at)
            .is_some_and(|&(held, _)| held == tick)
        {
            return;
        }
        if at == self.snapshots.len() {
            self.since_newest = 0;
        }
        self.snapshots.insert(at, (tick, karts));
        if self.render_tick.is_none() {
            // The first advance brings it to the delay.
            self.render_tick = Some(tick as f32 - DELAY_TICKS - 1.0);
        }
    }

    /// Run the render clock one client tick on.
    pub fn advance(&mut self) {
        let (Some(render), Some(&(newest, _))) = (self.render_tick, self.snapshots.back()) else {
            return;
        };
        let due = self.since_newest.min(TICKS_PER_SNAPSHOT - 1);
        let target = (newest + due) as f32 - DELAY_TICKS;
        self.since_newest += 1;
        let next = render + 1.0;
        let off = target - next;
        let render = if off.abs() > RESYNC_TICKS {
            target
        } else {
            next + (off * INTERPOLATION_CLOCK_GAIN)
                .clamp(-INTERPOLATION_CLOCK_SLEW, INTERPOLATION_CLOCK_SLEW)
        };
        let render = render.min(newest as f32);
        self.render_tick = Some(render);
        // Keep the last snapshot at or before the clock and everything after.
        while self.snapshots.len() > 1 && self.snapshots[1].0 as f32 <= render {
            self.snapshots.pop_front();
        }
    }

    /// The server tick the karts are drawn at, never past the newest
    /// snapshot received; `None` before the first.
    pub fn render_tick(&self) -> Option<f32> {
        self.render_tick
    }

    /// Every kart in grid order where the snapshots either side of the
    /// render clock put it; at the first held when the clock is before it.
    /// Empty before the first snapshot.
    pub fn kart_transforms(&self) -> Vec<KartTransform> {
        let Some(render) = self.render_tick else {
            return Vec::new();
        };
        let after = self
            .snapshots
            .iter()
            .position(|&(tick, _)| tick as f32 >= render);
        let (from, to, share) = match after {
            None | Some(0) => {
                let index = after.unwrap_or(self.snapshots.len() - 1);
                (index, index, 0.0)
            }
            Some(index) => {
                let (before, _) = self.snapshots[index - 1];
                let (after, _) = self.snapshots[index];
                let share = (render - before as f32) / (after - before) as f32;
                (index - 1, index, share)
            }
        };
        let (from_tick, from) = &self.snapshots[from];
        let (to_tick, to) = &self.snapshots[to];
        let ticks = to_tick - from_tick;
        from.iter()
            .zip(to)
            .map(|(from, to)| between(from, to, ticks, share))
            .collect()
    }
}

/// The kart `share` of the way from `from` to `to`, `ticks` apart; its
/// drift is `from`'s. A kart put somewhere new is at `from` until `to`.
fn between(from: &SnapshotKart, to: &SnapshotKart, ticks: u32, share: f32) -> KartTransform {
    let moved = Vec3::from(from.position).distance(Vec3::from(to.position));
    let share = if moved > ticks as f32 * FARTHEST_DRIVE_PER_TICK_M {
        share.floor()
    } else {
        share
    };
    let position = Vec3::from(from.position).lerp(Vec3::from(to.position), share);
    let rotation = Rotation::from_array(from.rotation)
        .slerp(Rotation::from_array(to.rotation), share)
        .normalize();
    let velocity = Vec3::from(from.velocity).lerp(Vec3::from(to.velocity), share);
    KartTransform {
        position: position.to_array(),
        rotation: rotation.to_array(),
        speed: velocity.dot(rotation * Vec3::NEG_Z),
        velocity: velocity.to_array(),
        drift: from.drift,
    }
}
