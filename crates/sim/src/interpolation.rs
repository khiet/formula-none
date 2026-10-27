//! The other karts as the client draws them (ADR 0009): between the two
//! snapshots it has received either side of a render clock that runs the
//! tuned delay behind the newest. The delay holds several snapshots, so one
//! lost or late leaves a pair to draw between and the karts move on as
//! smoothly as before.
//!
//! The render clock runs one tick per client tick, nudged toward the delay
//! behind the newest snapshot, so it follows the server's rate and not each
//! snapshot's jitter or loss. It never runs past the newest snapshot: what
//! the client has not received is never read, and a kart starved of
//! snapshots waits at the newest rather than guessing on.
//!
//! A kart that moved further between two snapshots than any kart can drive
//! was put there, by a respawn; it is drawn where it was until the snapshot
//! that has it in its new place, never streaking across the track between.
//!
//! The projectiles in flight are drawn the same way, between the two
//! snapshots, so one reaches its target as the target is drawn spinning.

use std::collections::VecDeque;

use rapier3d::prelude::*;

use crate::tuning::{
    INTERPOLATION_CLOCK_GAIN, INTERPOLATION_CLOCK_SLEW, INTERPOLATION_DELAY_S,
    INTERPOLATION_RESYNC_S, INTERPOLATION_TELEPORT_M_PER_TICK, SNAPSHOTS_PER_SECOND,
    TICKS_PER_SECOND,
};
use crate::{KartReadout, Projectile};

const DELAY_TICKS: f32 = INTERPOLATION_DELAY_S * TICKS_PER_SECOND as f32;
const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;
const RESYNC_TICKS: f32 = INTERPOLATION_RESYNC_S * TICKS_PER_SECOND as f32;

/// Every kart of a room's race, and every projectile in flight, as the
/// snapshots have it, drawn the tuned delay behind the newest.
#[derive(Default)]
pub struct Interpolation {
    /// The snapshots received, in tick order, from the last at or before the
    /// render clock on.
    snapshots: VecDeque<HeldSnapshot>,
    /// The server tick the karts are drawn at; `None` before a snapshot.
    render_tick: Option<f32>,
    /// Client ticks advanced since the newest snapshot arrived.
    since_newest: u32,
}

/// What one snapshot said, as far as the drawing reads it.
struct HeldSnapshot {
    tick: u32,
    karts: Vec<KartReadout>,
    projectiles: Vec<Projectile>,
}

impl Interpolation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold the snapshot of `tick`, which read `karts` for every kart in grid
    /// order and `projectiles` of every projectile in flight, in the order
    /// they were fired. One older than both the render clock and the
    /// earliest held, or already held, changes nothing; one that arrives
    /// after a newer one fills its place.
    pub fn push(&mut self, tick: u32, karts: Vec<KartReadout>, projectiles: Vec<Projectile>) {
        let passed = self
            .render_tick
            .is_some_and(|render| (tick as f32) < render)
            && self
                .snapshots
                .front()
                .is_some_and(|first| tick < first.tick);
        if passed {
            return;
        }
        let at = self.snapshots.partition_point(|held| held.tick < tick);
        if self.snapshots.get(at).is_some_and(|held| held.tick == tick) {
            return;
        }
        if at == self.snapshots.len() {
            self.since_newest = 0;
        }
        self.snapshots.insert(
            at,
            HeldSnapshot {
                tick,
                karts,
                projectiles,
            },
        );
        if self.render_tick.is_none() {
            // The first advance brings it to the delay.
            self.render_tick = Some(tick as f32 - DELAY_TICKS - 1.0);
        }
    }

    /// Run the render clock one client tick on.
    pub fn advance(&mut self) {
        let (Some(render), Some(newest)) = (self.render_tick, self.snapshots.back()) else {
            return;
        };
        let newest = newest.tick;
        // Counting the ticks since the newest arrived, up to the next one's
        // due, keeps the clock from being pulled back and forth at the
        // snapshot rate.
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
        // The clock never runs back, so only the earlier of the pair it is
        // between will be read again.
        while self.snapshots.len() > 1 && self.snapshots[1].tick as f32 <= render {
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
    pub fn kart_readouts(&self) -> Vec<KartReadout> {
        let Some((from, to, share)) = self.pair() else {
            return Vec::new();
        };
        let ticks = to.tick - from.tick;
        from.karts
            .iter()
            .zip(&to.karts)
            .map(|(from, to)| between(from, to, ticks, share))
            .collect()
    }

    /// Every projectile in flight where the snapshots either side of the
    /// render clock put it, in the earlier snapshot's order. One the later
    /// snapshot still carries, matched by owner and target in fired
    /// order, is drawn between the two; one it no longer carries reached
    /// its target between them and stays where the earlier had it until
    /// the clock passes the later. One fired between them is not drawn
    /// until the clock reaches the later. Empty before the first snapshot.
    pub fn projectiles(&self) -> Vec<Projectile> {
        let Some((from, to, share)) = self.pair() else {
            return Vec::new();
        };
        let mut later = to.projectiles.iter();
        from.projectiles
            .iter()
            .map(|earlier| {
                let position = later
                    .by_ref()
                    .find(|later| later.owner == earlier.owner && later.target == earlier.target)
                    .map_or(earlier.position, |later| {
                        Vec3::from(earlier.position)
                            .lerp(Vec3::from(later.position), share)
                            .to_array()
                    });
                Projectile {
                    position,
                    ..*earlier
                }
            })
            .collect()
    }

    /// The snapshots either side of the render clock and how far the clock
    /// is from the earlier to the later, as a share; the first held twice
    /// when the clock is before it, and the newest twice when past it.
    /// `None` before the first snapshot.
    fn pair(&self) -> Option<(&HeldSnapshot, &HeldSnapshot, f32)> {
        let render = self.render_tick?;
        let after = self
            .snapshots
            .iter()
            .position(|held| held.tick as f32 >= render);
        Some(match after {
            None | Some(0) => {
                let held = &self.snapshots[after.unwrap_or(self.snapshots.len() - 1)];
                (held, held, 0.0)
            }
            Some(index) => {
                let (before, after) = (&self.snapshots[index - 1], &self.snapshots[index]);
                let share = (render - before.tick as f32) / (after.tick - before.tick) as f32;
                (before, after, share)
            }
        })
    }
}

/// The kart `share` of the way from `from` to `to`, `ticks` apart: its
/// transform and velocity between the two, never airborne, and the rest,
/// its boost included, `from`'s. A kart put somewhere new is at `from` until `to`.
fn between(from: &KartReadout, to: &KartReadout, ticks: u32, share: f32) -> KartReadout {
    let moved = Vec3::from(from.position).distance(Vec3::from(to.position));
    let share = if moved > ticks as f32 * INTERPOLATION_TELEPORT_M_PER_TICK {
        share.floor()
    } else {
        share
    };
    let position = Vec3::from(from.position).lerp(Vec3::from(to.position), share);
    let rotation = Rotation::from_array(from.rotation)
        .slerp(Rotation::from_array(to.rotation), share)
        .normalize();
    let velocity = Vec3::from(from.velocity).lerp(Vec3::from(to.velocity), share);
    let rotation = rotation.to_array();
    let velocity = velocity.to_array();
    KartReadout {
        position: position.to_array(),
        rotation,
        speed: KartReadout::facing_speed(rotation, velocity),
        velocity,
        airborne: false,
        ..*from
    }
}
