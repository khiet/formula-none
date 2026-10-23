//! What the fake-transport client tests share: the browser's tick loop
//! reads and writes the wire's types, and these turn them into the sim's.
//! Each test binary compiles this module and uses only some of it.
#![allow(dead_code)]

use std::collections::BTreeMap;

use protocol::{
    ClientMessage, DriftDirection, Event, FinalStanding, Input, KartSnapshot, PlayerState,
    ServerMessage, Snapshot,
};
use server::fake_transport::Client;
use server::room::Started;
use sim::tuning::{BOT_LOOKAHEAD_M, BOT_START_HOLD_S, COUNTDOWN_S, TICKS_PER_SECOND};
use sim::{Drift, KartInput, Phase, Projectile, SnapshotKart, Track};

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

/// What one client has heard of a race: every event on the reliable lane,
/// in order, every snapshot by tick, and the results once they come.
#[derive(Default)]
pub struct Heard {
    pub events: Vec<Event>,
    pub snapshots: BTreeMap<u32, Snapshot>,
    pub results: Option<Vec<FinalStanding>>,
}

impl Heard {
    /// Take in everything `client` has received since the last call.
    pub fn hear(&mut self, client: &mut Client) {
        while let Some(message) = client.try_recv() {
            match message {
                ServerMessage::Snapshot(snapshot) => {
                    self.snapshots.insert(snapshot.tick, snapshot);
                }
                ServerMessage::Events(events) => self.events.extend(events),
                ServerMessage::Results { standings } => self.results = Some(standings),
                _ => {}
            }
        }
    }

    pub fn newest(&self) -> Option<&Snapshot> {
        self.snapshots.values().next_back()
    }
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
        held_item: kart.held_item.map(|item| match item {
            protocol::Item::Boost => sim::Item::Boost,
            protocol::Item::Hazard => sim::Item::Hazard,
            protocol::Item::Homing => sim::Item::Homing,
        }),
        spin_out_s: kart.spin_out_s,
        start: match kart.start {
            protocol::Start::Clean => sim::Start::Clean,
            protocol::Start::WeakBoost => sim::Start::WeakBoost,
            protocol::Start::FullBoost => sim::Start::FullBoost,
            protocol::Start::Stall => sim::Start::Stall,
        },
        stall_s: kart.stall_s,
        lap: u32::from(kart.lap),
        checkpoint: usize::from(kart.checkpoint),
        spline_progress: kart.spline_progress,
    }
}

/// The projectiles a snapshot carries, in the sim's terms.
pub fn projectiles(snapshot: &Snapshot) -> Vec<Projectile> {
    snapshot
        .projectiles
        .iter()
        .map(|projectile| Projectile {
            position: projectile.position,
            owner: usize::from(projectile.owner),
            target: usize::from(projectile.target),
        })
        .collect()
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

/// The rocket start as a bot times it, then `then`: through the countdown
/// the throttle is down from the tuned count, which earns the full boost at
/// Go, and once the race is on the input is `then`'s.
pub fn with_rocket_start(phase: Phase, then: KartInput) -> KartInput {
    match phase {
        Phase::Countdown { remaining_s } => KartInput {
            throttle: f32::from(u8::from(remaining_s <= BOT_START_HOLD_S)),
            ..KartInput::default()
        },
        _ => then,
    }
}

/// A test that needs the bots to pass idle karts clean pins a seed whose
/// race rolls no bot a rocket start; this holds the snapshots just before
/// Go to that, so a reseed that breaks the premise fails here and not in
/// the race it spoils. `humans` bots sit on the grid's front slots.
pub fn assert_no_bot_rocket_starts(snapshot: &Snapshot, humans: usize) {
    if (COUNTDOWN_S - 0.2..COUNTDOWN_S).contains(&seconds(snapshot.tick)) {
        assert!(
            snapshot.karts[humans..]
                .iter()
                .all(|kart| kart.start == protocol::Start::Clean),
            "a bot rolled a rocket start; the seed must roll none"
        );
    }
}

/// Full throttle along the bots' line, the tuned distance right of the
/// centreline, from where the snapshot has the kart.
pub fn follow_the_line(track: &Track, kart: &KartSnapshot) -> KartInput {
    const RIGHT_OF_CENTRE_M: f32 = 2.5;
    const STEER_GAIN: f32 = 4.0;
    let progress = kart.spline_progress;
    let ahead = track.point_at(progress + BOT_LOOKAHEAD_M);
    let further = track.point_at(progress + BOT_LOOKAHEAD_M + 1.0);
    // Right of the direction of travel (dx, dz), seen from above, is (-dz, dx).
    let (tx, tz) = (further[0] - ahead[0], further[2] - ahead[2]);
    let travel = (tx * tx + tz * tz).sqrt().max(f32::EPSILON);
    let target = [
        ahead[0] - tz / travel * RIGHT_OF_CENTRE_M,
        ahead[2] + tx / travel * RIGHT_OF_CENTRE_M,
    ];
    let (dx, dz) = (target[0] - kart.position[0], target[1] - kart.position[2]);
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    let [qx, qy, qz, qw] = kart.rotation;
    let yaw = (2.0 * (qw * qy + qx * qz)).atan2(1.0 - 2.0 * (qy * qy + qz * qz));
    // Positive steer turns the kart right; right of a yaw is (cos, -sin) in x, z.
    let (sin, cos) = yaw.sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    KartInput {
        steer: (rightward * STEER_GAIN).clamp(-1.0, 1.0),
        throttle: 1.0,
        ..KartInput::default()
    }
}
