//! What the fake-transport client tests share: the browser's tick loop
//! reads and writes the wire's types, and these turn them into the sim's.
//! Each test binary compiles this module and uses only some of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::time::Duration;

use protocol::{
    ClientMessage, Event, EventKind, FinalStanding, Input, KartSnapshot, PlayerState,
    ServerMessage, Snapshot,
};
use server::fake_transport::Client;
use server::room::Started;
use sim::tuning::{BOT_LOOKAHEAD_M, BOT_START_HOLD_S, COUNTDOWN_S, TICKS_PER_SECOND};
use sim::{KartInput, Phase, Projectile, Track};

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

/// The projectiles a snapshot carries, in the sim's terms.
pub fn projectiles(snapshot: &Snapshot) -> Vec<Projectile> {
    snapshot.projectiles.iter().map(Projectile::from).collect()
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
/// the throttle is held from `BOT_START_HOLD_S`, which earns the full boost
/// at Go, and from Go the input is `then`.
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
/// race rolls no bot the start boost; this holds the snapshots just before
/// Go to that, so a reseed that breaks the premise fails here and not in
/// the race it spoils. The first `humans` slots are the humans'.
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
    KartInput {
        steer: steer_toward(kart.position, kart.rotation, target),
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// The steer that turns a kart at `position`, facing by `rotation`, toward
/// `target` on the ground: full lock from about 15 degrees off, easing to
/// straight on it.
pub fn steer_toward(position: [f32; 3], rotation: [f32; 4], target: [f32; 2]) -> f32 {
    const STEER_GAIN: f32 = 4.0;
    let (dx, dz) = (target[0] - position[0], target[1] - position[2]);
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    let [qx, qy, qz, qw] = rotation;
    let yaw = (2.0 * (qw * qy + qx * qz)).atan2(1.0 - 2.0 * (qy * qy + qz * qz));
    // Positive steer turns the kart right; right of a yaw is (cos, -sin) in x, z.
    let (sin, cos) = yaw.sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    (rightward * STEER_GAIN).clamp(-1.0, 1.0)
}

const OVAL: &str = include_str!("../../../../assets/tracks/oval/oval.track.json");

/// What one player heard of a race until its results: every event on the
/// reliable lane with the tick of the newest snapshot when it came, the
/// results, and the newest snapshot's tick when they came.
pub struct Raced {
    pub events: Vec<(u32, Event)>,
    pub results: Vec<FinalStanding>,
    pub results_tick: u32,
}

impl Raced {
    pub fn highest_id(&self) -> u32 {
        self.events
            .iter()
            .map(|(_, event)| event.id)
            .max()
            .unwrap_or(0)
    }

    /// The tick `kart` finished at, as this player heard it.
    pub fn finish_tick(&self, kart: u8) -> Option<u32> {
        self.events
            .iter()
            .find_map(|&(tick, event)| match event.kind {
                EventKind::Finished { kart: finisher, .. } if finisher == kart => Some(tick),
                _ => None,
            })
    }
}

/// Each of `players` drives its kart in `drivers`, the kart and the seconds
/// after Go it sets off at, along the bots' line until the results reach
/// it, sending an input a tick as the browser does. Whatever comes after
/// the results is left unread.
pub async fn drive_to_results(players: &mut [Client], drivers: &[(usize, f32)]) -> Vec<Raced> {
    const DEADLINE_S: f32 = 240.0;
    let tick = Duration::from_nanos(1_000_000_000 / u64::from(TICKS_PER_SECOND));
    let track = Track::from_sidecar(OVAL).expect("the oval fixture loads");
    let count = players.len();
    let mut events: Vec<Vec<(u32, Event)>> = vec![Vec::new(); count];
    let mut newest: Vec<Option<Snapshot>> = vec![None; count];
    let mut raced: Vec<Option<Raced>> = (0..count).map(|_| None).collect();
    let mut sequences = vec![0; count];
    let mut ticks = tokio::time::interval(tick);
    while raced.iter().any(Option::is_none) {
        ticks.tick().await;
        for (index, player) in players.iter_mut().enumerate() {
            while raced[index].is_none() {
                let Some(message) = player.try_recv() else {
                    break;
                };
                let at = newest[index].as_ref().map_or(0, |snapshot| snapshot.tick);
                match message {
                    ServerMessage::Snapshot(snapshot) => newest[index] = Some(snapshot),
                    ServerMessage::Events(heard) => {
                        events[index].extend(heard.into_iter().map(|event| (at, event)));
                    }
                    ServerMessage::Results { standings } => {
                        raced[index] = Some(Raced {
                            events: std::mem::take(&mut events[index]),
                            results: standings,
                            results_tick: at,
                        });
                    }
                    _ => {}
                }
            }
            let Some(snapshot) = &newest[index] else {
                continue;
            };
            let elapsed = seconds(snapshot.tick);
            assert!(
                elapsed < DEADLINE_S,
                "the race did not end in {DEADLINE_S} s"
            );
            let (kart, set_off_s) = drivers[index];
            if raced[index].is_some() || elapsed < COUNTDOWN_S + set_off_s {
                continue;
            }
            sequences[index] += 1;
            let input = follow_the_line(&track, &snapshot.karts[kart]);
            player.send_input(wire(sequences[index], input));
        }
    }
    raced.into_iter().map(Option::unwrap).collect()
}
