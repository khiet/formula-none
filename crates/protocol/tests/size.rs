//! The wire budgets from ADR 0010: an input near 8 bytes at 60 Hz, a
//! snapshot near 400 bytes at eight karts and 30 Hz, grown to what the
//! ADR's Outcome records as the rules landed. Held with a tolerance so a
//! field can be added without moving the budget, and so the budget still
//! catches a field that does not belong.

use protocol::{encode, samples, ClientMessage, Input, ServerMessage, Snapshot};

/// The budget is "roughly" this many bytes.
const INPUT_BUDGET: usize = 8;
/// ADR 0010 estimated 400; its Outcome records the busy sample at 505
/// bytes once the hazards and the projectiles rode along, and holds it
/// there.
const SNAPSHOT_BUDGET: usize = 500;

/// A quarter either way.
fn within_tolerance(size: usize, budget: usize) -> bool {
    (budget * 3 / 4..=budget * 5 / 4).contains(&size)
}

#[test]
fn an_input_message_is_roughly_eight_bytes() {
    // Ten minutes in, when the sequence takes three bytes.
    let message = ClientMessage::Input(Input {
        sequence: 60 * 60 * 10,
        ..samples::input()
    });
    let size = encode(&message).len();
    assert!(within_tolerance(size, INPUT_BUDGET), "{size} bytes");
}

#[test]
fn a_snapshot_for_eight_karts_is_roughly_five_hundred_bytes() {
    let snapshot = samples::snapshot();
    assert_eq!(snapshot.karts.len(), 8);
    let size = encode(&ServerMessage::Snapshot(snapshot)).len();
    assert!(within_tolerance(size, SNAPSHOT_BUDGET), "{size} bytes");
}

#[test]
fn the_snapshot_sample_is_a_busy_mid_race_moment() {
    // The budget is only meaningful against a snapshot with something in it:
    // karts drifting and holding items, one spun out, events to repeat.
    let Snapshot {
        karts,
        events,
        hazards,
        projectiles,
        ..
    } = samples::snapshot();
    assert!(karts.iter().any(|kart| kart.drift.is_some()));
    assert!(karts.iter().any(|kart| kart.held_item.is_some()));
    assert!(karts.iter().any(|kart| kart.spin_out_s.is_some()));
    assert!(!events.is_empty());
    assert!(!hazards.is_empty());
    assert!(!projectiles.is_empty());
}
