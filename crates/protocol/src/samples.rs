//! One value of every message type. The Rust round-trip tests run over
//! them, and the generator writes them next to the TypeScript types as
//! fixtures, so Vitest decodes the bytes the Rust encoder produced for every
//! message type (ADR 0004). A new variant needs a sample here: the Vitest
//! side holds the fixtures to the generated union and fails to compile
//! until it has one.

use crate::{
    ClientMessage, DriftDirection, DriftSnapshot, Event, EventKind, FinalStanding, Input, Item,
    KartSnapshot, Player, PlayerState, Refusal, Room, RoomState, ServerMessage, Snapshot,
    PROTOCOL_VERSION,
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
            track: "brands-patch".to_owned(),
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
/// five holding items, one spun out, and every kind of event since the
/// previous snapshot. The byte budget in ADR 0010 is measured against this.
pub fn snapshot() -> Snapshot {
    let karts = (0..8u8)
        .map(|slot| {
            let along = f32::from(slot) * 7.5;
            KartSnapshot {
                position: [12.5 + along, 0.3, -40.0 - along * 0.5],
                rotation: [0.0, 0.3826834, 0.0, 0.9238795],
                velocity: [21.0, 0.0, -21.0],
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
                held_item: match slot % 3 {
                    0 => Some(Item::Hazard),
                    1 => None,
                    _ => Some(Item::Boost),
                },
                spin_out_s: (slot == 6).then_some(0.5),
            }
        })
        .collect();
    Snapshot {
        tick: 7200,
        acknowledged: vec![7195, 7198, 0, 0, 0, 0, 0, 0],
        karts,
        events: every_event_kind(),
    }
}

/// Every [`EventKind`] once, as the last tick of the sample snapshot, so the
/// generated `EventKind` union is checked against bytes for each kind.
fn every_event_kind() -> Vec<Event> {
    [
        EventKind::ItemPicked {
            kart: 3,
            item: Item::Homing,
        },
        EventKind::ItemFired {
            kart: 3,
            item: Item::Homing,
        },
        EventKind::Hit { kart: 6, by: 1 },
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
    ]
    .into_iter()
    .zip(41..)
    .map(|(kind, id)| Event { id, kind })
    .collect()
}
