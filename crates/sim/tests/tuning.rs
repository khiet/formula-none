//! Invariants of the item roll table. Tests read the values from the module
//! rather than repeating them, so retuning never breaks them unless it breaks
//! a rule. Invariants between plain constants are `const` asserts in the
//! module itself.

use sim::tuning::{self, ITEM_ROLL_PERCENT, ITEM_ROLL_WEIGHTS};
use sim::BotLevel;

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
fn each_item_roll_band_sums_to_the_rolls_percent() {
    for band in ITEM_ROLL_WEIGHTS {
        let total = u32::from(band.hazard)
            + u32::from(band.boost)
            + u32::from(band.homing)
            + u32::from(band.star)
            + u32::from(band.horn);
        assert_eq!(total, ITEM_ROLL_PERCENT, "band {:?}", band.positions);
    }
}

#[test]
fn the_basic_bot_profile_is_the_named_constants_so_a_retune_still_moves_basic() {
    let basic = BotLevel::Basic.profile();
    assert_eq!(
        basic.corner_speed_fraction,
        tuning::BOT_CORNER_SPEED_FRACTION
    );
    assert_eq!(
        basic.rubber_band_min_factor,
        tuning::BOT_RUBBER_BAND_MIN_FACTOR
    );
    assert_eq!(
        basic.rubber_band_max_factor,
        tuning::BOT_RUBBER_BAND_MAX_FACTOR
    );
    assert_eq!(basic.item_use_delay_s, tuning::BOT_ITEM_USE_DELAY_S);
    assert_eq!(basic.start_boost_chance, tuning::BOT_START_BOOST_CHANCE);
    assert_eq!(BotLevel::default(), BotLevel::Basic);
}

#[test]
fn each_bot_level_above_basic_drives_harder_on_every_profile_value_that_varies() {
    let [basic, pro, ace] = [BotLevel::Basic, BotLevel::Pro, BotLevel::Ace].map(BotLevel::profile);
    assert!(basic.corner_speed_fraction < pro.corner_speed_fraction);
    assert!(pro.corner_speed_fraction < ace.corner_speed_fraction);
    assert!(basic.rubber_band_min_factor < pro.rubber_band_min_factor);
    assert!(pro.rubber_band_min_factor < ace.rubber_band_min_factor);
    assert!(basic.item_use_delay_s > pro.item_use_delay_s);
    assert!(pro.item_use_delay_s > ace.item_use_delay_s);
    assert!(basic.start_boost_chance < pro.start_boost_chance);
    assert!(pro.start_boost_chance < ace.start_boost_chance);
    // An Ace bot ahead of every human is never slowed, and the band's top
    // stays where the slipstream's assert holds it (ADR 0077).
    assert_eq!(ace.rubber_band_min_factor, 1.0);
    assert_eq!(ace.start_boost_chance, 1.0);
    assert_eq!(ace.item_use_delay_s, 0.0);
    assert_eq!(ace.rubber_band_max_factor, basic.rubber_band_max_factor);
    assert_eq!(pro.rubber_band_max_factor, basic.rubber_band_max_factor);
}
