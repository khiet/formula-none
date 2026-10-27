//! The wire's types for the sim's and back. What the wire leaves out comes
//! back as the sim reports a kart only a snapshot speaks for: never
//! airborne, going at the speed its velocity has along its facing.

use sim::KartReadout;

use crate::{
    BoostSnapshot, BoostTier, DriftDirection, DriftSnapshot, EventKind, HazardSnapshot, Input,
    Item, KartSnapshot, ProjectileSnapshot, Start,
};

impl From<sim::Item> for Item {
    fn from(item: sim::Item) -> Self {
        match item {
            sim::Item::Boost => Self::Boost,
            sim::Item::Hazard => Self::Hazard,
            sim::Item::Homing => Self::Homing,
        }
    }
}

impl From<Item> for sim::Item {
    fn from(item: Item) -> Self {
        match item {
            Item::Boost => Self::Boost,
            Item::Hazard => Self::Hazard,
            Item::Homing => Self::Homing,
        }
    }
}

impl From<sim::Start> for Start {
    fn from(start: sim::Start) -> Self {
        match start {
            sim::Start::Clean => Self::Clean,
            sim::Start::WeakBoost => Self::WeakBoost,
            sim::Start::FullBoost => Self::FullBoost,
            sim::Start::Stall => Self::Stall,
        }
    }
}

impl From<Start> for sim::Start {
    fn from(start: Start) -> Self {
        match start {
            Start::Clean => Self::Clean,
            Start::WeakBoost => Self::WeakBoost,
            Start::FullBoost => Self::FullBoost,
            Start::Stall => Self::Stall,
        }
    }
}

impl From<sim::DriftDirection> for DriftDirection {
    fn from(direction: sim::DriftDirection) -> Self {
        match direction {
            sim::DriftDirection::Left => Self::Left,
            sim::DriftDirection::Right => Self::Right,
        }
    }
}

impl From<DriftDirection> for sim::DriftDirection {
    fn from(direction: DriftDirection) -> Self {
        match direction {
            DriftDirection::Left => Self::Left,
            DriftDirection::Right => Self::Right,
        }
    }
}

impl From<sim::Drift> for DriftSnapshot {
    fn from(drift: sim::Drift) -> Self {
        Self {
            direction: drift.direction.into(),
            charge_s: drift.charge_s,
        }
    }
}

impl From<DriftSnapshot> for sim::Drift {
    fn from(drift: DriftSnapshot) -> Self {
        Self {
            direction: drift.direction.into(),
            charge_s: drift.charge_s,
        }
    }
}

/// The tier of a boost under way. The sim reports `MiniTurbo::None` only
/// off a boost, which the wire leaves out, so it never reaches here; it
/// reads as red, the tier of every boost but an orange one.
impl From<sim::MiniTurbo> for BoostTier {
    fn from(tier: sim::MiniTurbo) -> Self {
        match tier {
            sim::MiniTurbo::Orange => Self::Orange,
            sim::MiniTurbo::Red | sim::MiniTurbo::None => Self::Red,
        }
    }
}

impl From<BoostTier> for sim::MiniTurbo {
    fn from(tier: BoostTier) -> Self {
        match tier {
            BoostTier::Orange => Self::Orange,
            BoostTier::Red => Self::Red,
        }
    }
}

/// What a snapshot carries of a kart: the readout less what only the sim
/// that steps the kart reports.
impl From<&KartReadout> for KartSnapshot {
    fn from(kart: &KartReadout) -> Self {
        Self {
            position: kart.position,
            rotation: kart.rotation,
            velocity: kart.velocity,
            drift: kart.drift.map(DriftSnapshot::from),
            race_position: kart.race_position as u8,
            lap: kart.lap as u8,
            checkpoint: kart.checkpoint as u8,
            spline_progress: kart.spline_progress,
            wrong_way: kart.wrong_way,
            held_item: kart.held_item.map(Item::from),
            spin_out_s: kart.spin_out_s,
            start: kart.start.into(),
            stall_s: kart.stall_s,
            boost: kart.boost_s.map(|left_s| BoostSnapshot {
                left_s,
                tier: kart.boost_tier.into(),
            }),
        }
    }
}

impl From<&KartSnapshot> for KartReadout {
    fn from(kart: &KartSnapshot) -> Self {
        let drift = kart.drift.map(sim::Drift::from);
        Self {
            position: kart.position,
            rotation: kart.rotation,
            speed: KartReadout::facing_speed(kart.rotation, kart.velocity),
            velocity: kart.velocity,
            drift,
            mini_turbo: sim::MiniTurbo::of(drift),
            held_item: kart.held_item.map(sim::Item::from),
            spin_out_s: kart.spin_out_s,
            airborne: false,
            boost_s: kart.boost.map(|boost| boost.left_s),
            boost_tier: kart
                .boost
                .map_or(sim::MiniTurbo::None, |boost| boost.tier.into()),
            start: kart.start.into(),
            stall_s: kart.stall_s,
            race_position: u32::from(kart.race_position),
            lap: u32::from(kart.lap),
            checkpoint: usize::from(kart.checkpoint),
            spline_progress: kart.spline_progress,
            wrong_way: kart.wrong_way,
        }
    }
}

/// A sim event as the wire tells it. The sim's spin-out is the wire's
/// [`EventKind::Hit`]: the players are told who hit whom, and the spin-out
/// it caused rides the snapshots as the time left.
impl From<sim::RaceEvent> for EventKind {
    fn from(event: sim::RaceEvent) -> Self {
        match event {
            sim::RaceEvent::LapCompleted {
                kart,
                lap,
                lap_time_s,
            } => Self::LapCompleted {
                kart: kart as u8,
                lap: lap as u8,
                lap_time_s,
            },
            sim::RaceEvent::Finished {
                kart,
                position,
                finish_time_s,
            } => Self::Finished {
                kart: kart as u8,
                position: position as u8,
                finish_time_s,
            },
            sim::RaceEvent::Respawned { kart } => Self::Respawned { kart: kart as u8 },
            sim::RaceEvent::ItemPicked {
                kart,
                item,
                item_box,
            } => Self::ItemPicked {
                kart: kart as u8,
                item: item.into(),
                item_box: item_box as u8,
            },
            sim::RaceEvent::ItemFired { kart, item } => Self::ItemFired {
                kart: kart as u8,
                item: item.into(),
            },
            sim::RaceEvent::SpunOut { kart, by } => Self::Hit {
                kart: kart as u8,
                by: by as u8,
            },
            sim::RaceEvent::RaceEnded => Self::RaceEnded,
        }
    }
}

impl From<&sim::Hazard> for HazardSnapshot {
    fn from(hazard: &sim::Hazard) -> Self {
        Self {
            position: hazard.position,
            owner: hazard.owner as u8,
            held: hazard.held,
        }
    }
}

impl From<&sim::Projectile> for ProjectileSnapshot {
    fn from(projectile: &sim::Projectile) -> Self {
        Self {
            position: projectile.position,
            owner: projectile.owner as u8,
            target: projectile.target as u8,
        }
    }
}

impl From<&ProjectileSnapshot> for sim::Projectile {
    fn from(projectile: &ProjectileSnapshot) -> Self {
        Self {
            position: projectile.position,
            owner: usize::from(projectile.owner),
            target: usize::from(projectile.target),
        }
    }
}

/// The controls as the sim reads them, each byte back in the unit range:
/// the server and the client's prediction step the same kart (ADR 0009).
impl From<Input> for sim::KartInput {
    fn from(input: Input) -> Self {
        Self {
            steer: input.steer_unit(),
            throttle: input.throttle_unit(),
            brake: input.brake_unit(),
            hop: input.hop,
            use_item: input.use_item,
            hold_item: input.hold_item,
        }
    }
}
