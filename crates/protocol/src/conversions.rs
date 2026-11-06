//! The wire's types for the sim's and back, for what is not a kart: a kart
//! crosses as its readout (ADR 0090).

use crate::{EventKind, HazardSnapshot, Input, ProjectileSnapshot};

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
                item,
                item_box: item_box as u8,
            },
            sim::RaceEvent::ItemFired { kart, item } => Self::ItemFired {
                kart: kart as u8,
                item,
            },
            sim::RaceEvent::CoinPicked { kart, coin } => Self::CoinPicked {
                kart: kart as u8,
                coin: coin as u8,
            },
            sim::RaceEvent::SpunOut { kart, by } => Self::Hit {
                kart: kart as u8,
                by: by.map(|by| by as u8),
            },
            sim::RaceEvent::CoinsLost { kart, count } => Self::CoinsLost {
                kart: kart as u8,
                count: count as u8,
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
