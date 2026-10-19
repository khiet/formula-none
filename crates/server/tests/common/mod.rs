//! What the fake-transport client tests share: the browser's tick loop
//! reads and writes the wire's types, and these turn them into the sim's.
//! Each test binary compiles this module and uses only some of it.
#![allow(dead_code)]

use protocol::{ClientMessage, DriftDirection, Input, KartSnapshot, PlayerState, Snapshot};
use server::fake_transport::Client;
use server::room::Started;
use sim::tuning::TICKS_PER_SECOND;
use sim::{Drift, KartInput, SnapshotKart};

/// Every one of `players` joins `code`, in order, and readies; once the
/// first sees them all ready it starts the race. Returns each one's
/// Countdown, in the same order.
pub async fn race_together(code: &str, players: &mut [Client]) -> Vec<Started> {
    for (index, player) in players.iter_mut().enumerate() {
        player.join(code, &format!("player{index}")).await.unwrap();
    }
    for player in players.iter() {
        player.send(&ClientMessage::Ready { ready: true });
    }
    while !players[0]
        .next_room()
        .await
        .unwrap()
        .players
        .iter()
        .all(|player| player.state == PlayerState::Ready)
    {}
    players[0].send(&ClientMessage::Start);
    let mut countdowns = Vec::new();
    for player in players.iter_mut() {
        countdowns.push(player.next_countdown().await.unwrap());
    }
    countdowns
}

/// The wire input for the sim's: each control quantised to its byte, as the
/// browser's room client sends it.
pub fn wire(sequence: u32, input: KartInput) -> Input {
    let unit = |value: f32| (value.clamp(0.0, 1.0) * f32::from(u8::MAX)).round() as u8;
    Input {
        sequence,
        steer: (input.steer.clamp(-1.0, 1.0) * f32::from(i8::MAX)).round() as i8,
        throttle: unit(input.throttle),
        brake: unit(input.brake),
        hop: input.hop,
        use_item: input.use_item,
        hold_item: input.hold_item,
    }
}

/// The sim's input for a wire input, as the server reads it.
pub fn read(input: Input) -> KartInput {
    KartInput {
        steer: input.steer_unit(),
        throttle: input.throttle_unit(),
        brake: input.brake_unit(),
        hop: input.hop,
        use_item: input.use_item,
        hold_item: input.hold_item,
    }
}

pub fn snapshot_kart(kart: &KartSnapshot) -> SnapshotKart {
    SnapshotKart {
        position: kart.position,
        rotation: kart.rotation,
        velocity: kart.velocity,
        drift: kart.drift.map(|drift| Drift {
            direction: match drift.direction {
                DriftDirection::Left => sim::DriftDirection::Left,
                DriftDirection::Right => sim::DriftDirection::Right,
            },
            charge_s: drift.charge_s,
        }),
        lap: u32::from(kart.lap),
        checkpoint: usize::from(kart.checkpoint),
        spline_progress: kart.spline_progress,
    }
}

/// A kart's speed in a snapshot, in metres per second.
pub fn speed(snapshot: &Snapshot, kart: usize) -> f32 {
    let [x, y, z] = snapshot.karts[kart].velocity;
    (x * x + y * y + z * z).sqrt()
}

/// Ticks on the race clock in seconds.
pub fn seconds(ticks: u32) -> f32 {
    ticks as f32 / TICKS_PER_SECOND as f32
}
