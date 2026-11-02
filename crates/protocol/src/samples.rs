//! One value of every message type. The Rust round-trip tests run over
//! them, and the generator writes them next to the TypeScript types as
//! fixtures, so Vitest decodes the bytes the Rust encoder produced for every
//! message type (ADR 0004). A new variant needs a sample here: the Vitest
//! side holds the fixtures to the generated union and fails to compile
//! until it has one.

use crate::{
    BoostSnapshot, BoostTier, ClientMessage, DriftDirection, DriftSnapshot, Event, EventKind,
    FinalStanding, HazardSnapshot, Input, Item, ItemSlot, KartSnapshot, Player, PlayerState,
    ProjectileSnapshot, Refusal, Room, RoomState, ServerMessage, Snapshot, Start, PROTOCOL_VERSION,
};

/// Every [`ClientMessage`] variant once.
pub fn client() -> Vec<ClientMessage> {
    vec![
        ClientMessage::Ping { nonce: 0xdead_beef },
        ClientMessage::Join {
            version: PROTOCOL_VERSION,
            room: "BXQF".to_owned(),
            nickname: "khiet".to_owned(),
        },
        ClientMessage::Input(input()),
        ClientMessage::Ready { ready: true },
        ClientMessage::Start,
        ClientMessage::Rematch,
        ClientMessage::Leave,
        ClientMessage::Rejoin {
            version: PROTOCOL_VERSION,
            room: "BXQF".to_owned(),
            token: token(),
        },
        ClientMessage::Pong { nonce: 7 },
        ClientMessage::RaceReport {
            corrections_per_second: 0.25,
        },
    ]
}

/// Every [`ServerMessage`] variant once.
pub fn server() -> Vec<ServerMessage> {
    vec![
        ServerMessage::Pong { nonce: 0xdead_beef },
        ServerMessage::Joined {
            slot: 1,
            room: room(),
            token: token(),
        },
        ServerMessage::Refused(Refusal::VersionMismatch {
            server: PROTOCOL_VERSION,
        }),
        ServerMessage::Room(room()),
        ServerMessage::Countdown {
            seed: 0x0123_4567_89ab_cdef,
            track: "bell-deer".to_owned(),
            grid: vec![0, 1],
        },
        ServerMessage::Snapshot(snapshot()),
        ServerMessage::Events(every_event_kind()),
        ServerMessage::Results {
            standings: vec![
                FinalStanding {
                    kart: 1,
                    position: 1,
                    finish_time_s: Some(121.5),
                    best_lap_s: Some(39.25),
                },
                FinalStanding {
                    kart: 0,
                    position: 2,
                    finish_time_s: None,
                    best_lap_s: None,
                },
            ],
        },
        ServerMessage::Replaced,
        ServerMessage::Ping { nonce: 7 },
    ]
}

/// A session token as the server deals them: 128 random bits in hex.
pub fn token() -> String {
    "0123456789abcdef0123456789abcdef".to_owned()
}

/// Mid-corner, hopping, item button just pressed.
pub fn input() -> Input {
    Input {
        sequence: 1234,
        steer: -64,
        throttle: 255,
        brake: 0,
        hop: true,
        use_item: true,
        hold_item: false,
    }
}

/// Two humans in an open room; the first is the creator and ready.
pub fn room() -> Room {
    Room {
        code: "BXQF".to_owned(),
        track: "goldstone".to_owned(),
        state: RoomState::Open,
        players: vec![
            Player {
                slot: 0,
                nickname: "khiet".to_owned(),
                livery: 0,
                state: PlayerState::Ready,
                creator: true,
            },
            Player {
                slot: 1,
                nickname: "ada".to_owned(),
                livery: 1,
                state: PlayerState::Unready,
                creator: false,
            },
        ],
    }
}

/// A busy moment two minutes into a race with eight karts: two drifting,
/// two boosting, five holding items, one of them a star, all but the last
/// carrying coins, one spun out, one just bumped, one on a star's run, two of fifteen coins taken, a
/// hazard trailed behind a kart, a projectile in flight, and every kind of
/// event since the previous snapshot. The byte budget in ADR 0010 is
/// measured against this.
pub fn snapshot() -> Snapshot {
    let karts = (0..8u8)
        .map(|slot| {
            let along = f32::from(slot) * 7.5;
            KartSnapshot {
                position: [12.5 + along, 0.3, -40.0 - along * 0.5],
                rotation: [0.0, 0.3826834, 0.0, 0.9238795],
                velocity: [21.0, 0.0, -21.0],
                bump: (slot == 3).then_some([-2.5, 0.0, -2.5]),
                drift: (slot % 4 == 1).then_some(DriftSnapshot {
                    direction: if slot < 4 {
                        DriftDirection::Left
                    } else {
                        DriftDirection::Right
                    },
                    charge_s: 0.75,
                }),
                race_position: slot + 1,
                lap: 2,
                checkpoint: 11 - slot,
                spline_progress: 640.0 - along * 10.0,
                wrong_way: slot == 5,
                item_slot: match slot % 4 {
                    0 => ItemSlot::Held(Item::Hazard),
                    1 => ItemSlot::Empty,
                    2 => ItemSlot::Rolling {
                        item: Item::Homing,
                        roll_left_s: 1.25,
                    },
                    _ if slot == 7 => ItemSlot::Held(Item::Star),
                    _ => ItemSlot::Held(Item::Boost),
                },
                coins: 7 - slot.min(7),
                spin_out_s: (slot == 6).then_some(0.5),
                start: match slot % 4 {
                    0 => Start::Clean,
                    1 => Start::WeakBoost,
                    2 => Start::FullBoost,
                    _ => Start::Stall,
                },
                stall_s: (slot == 7).then_some(1.2),
                boost: match slot {
                    2 => Some(BoostSnapshot {
                        left_s: 0.4,
                        tier: BoostTier::Orange,
                    }),
                    4 => Some(BoostSnapshot {
                        left_s: 1.1,
                        tier: BoostTier::Red,
                    }),
                    _ => None,
                },
                star_s: (slot == 5).then_some(4.5),
            }
        })
        .collect();
    Snapshot {
        tick: 7200,
        acknowledged: vec![7195, 7198, 0, 0, 0, 0, 0, 0],
        karts,
        events: every_event_kind(),
        hazards: vec![HazardSnapshot {
            position: [20.5, 0.0, -44.5],
            owner: 3,
            held: true,
        }],
        projectiles: vec![ProjectileSnapshot {
            position: [40.0, 0.0, -55.0],
            owner: 3,
            target: 2,
        }],
        coins_present: (0..15).map(|coin| coin != 4 && coin != 9).collect(),
    }
}

/// Every [`EventKind`] once, as the last tick of the sample snapshot, so the
/// generated `EventKind` union is checked against bytes for each kind.
fn every_event_kind() -> Vec<Event> {
    [
        EventKind::ItemPicked {
            kart: 3,
            item: Some(Item::Homing),
            item_box: 5,
        },
        // A kart already holding an item takes the box and rolls nothing.
        EventKind::ItemPicked {
            kart: 0,
            item: None,
            item_box: 2,
        },
        EventKind::ItemFired {
            kart: 3,
            item: Item::Homing,
        },
        // Another kart's horn, which every client acts on.
        EventKind::ItemFired {
            kart: 0,
            item: Item::Horn,
        },
        EventKind::CoinPicked { kart: 1, coin: 9 },
        EventKind::Hit {
            kart: 6,
            by: Some(1),
        },
        EventKind::CoinsLost { kart: 6, count: 3 },
        EventKind::Hit { kart: 2, by: None },
        EventKind::LapCompleted {
            kart: 0,
            lap: 2,
            lap_time_s: 41.5,
        },
        EventKind::Finished {
            kart: 7,
            position: 1,
            finish_time_s: 120.5,
        },
        EventKind::Respawned { kart: 4 },
        EventKind::RaceEnded,
    ]
    .into_iter()
    .zip(41..)
    .map(|(kind, id)| Event { id, kind })
    .collect()
}
