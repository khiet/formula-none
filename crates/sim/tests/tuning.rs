//! Invariants of the item roll table. Tests read the values from the module
//! rather than repeating them, so retuning never breaks them unless it breaks
//! a rule. Invariants between plain constants are `const` asserts in the
//! module itself.

use sim::tuning::{self, ITEM_ROLL_WEIGHTS};

#[test]
fn item_roll_weights_cover_every_grid_slot_once() {
    let mut next_position = 1;
    for band in ITEM_ROLL_WEIGHTS {
        let (first, last) = band.positions;
        assert_eq!(first, next_position, "bands must be contiguous");
        assert!(first <= last);
        next_position = last + 1;
    }
    assert_eq!(usize::from(next_position - 1), tuning::KARTS_PER_RACE);
}

#[test]
fn each_item_roll_band_sums_to_one_hundred_percent() {
    for band in ITEM_ROLL_WEIGHTS {
        let total = u32::from(band.hazard) + u32::from(band.boost) + u32::from(band.homing);
        assert_eq!(total, 100, "band {:?}", band.positions);
    }
}
