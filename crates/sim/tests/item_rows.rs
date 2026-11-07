//! Where the box rows stand, at the sim's public seam: bots race a course
//! from the grid, and each one that rolls from a row holds the item it
//! rolled before the next corner, so the item is there for the fight the
//! row sets up (ADR 0097).

mod common;

use common::{bell_deer, goldstone, started, ticks, DT, SEED};
use sim::{step, BotLevel, ItemSlot, Phase, RaceEvent, State, Track};

/// Tighter than this radius, the centreline is a corner.
const CORNER_RADIUS_M: f32 = 100.0;
/// Long enough for every bot to pass the row with an empty slot at least once.
const RACE_S: f32 = 70.0;

/// The boxes of the row whose middle is `row_progress` along the lap, and
/// the spline progress of the first corner past it.
fn row_and_next_corner(track: &Track, row_progress: f32) -> (Vec<usize>, f32) {
    let boxes: Vec<usize> = (0..track.item_boxes().len())
        .filter(|&b| (track.progress_at(track.item_boxes()[b].position) - row_progress).abs() < 2.0)
        .collect();
    assert_eq!(boxes.len(), 4, "a row of four boxes at {row_progress} m");
    let mut corner = row_progress;
    while track.curvature_at(corner).abs() < 1.0 / CORNER_RADIUS_M {
        corner += 0.5;
    }
    (boxes, corner)
}

/// Where along the lap each bot that rolled from `boxes` held its item, in a
/// race of eight bots at `level`.
fn landings(course: fn() -> Track, level: BotLevel, boxes: &[usize]) -> Vec<f32> {
    let mut found = Vec::new();
    let mut state = started(State::with_bots(SEED, 0, 8, course(), level));
    let mut rolling_from_row = [false; 8];
    for _ in 0..ticks(RACE_S) {
        state = step(state, &[], DT);
        if !matches!(state.phase(), Phase::Racing { .. }) {
            break;
        }
        for event in state.events() {
            if let RaceEvent::ItemPicked {
                kart,
                item: Some(_),
                item_box,
            } = *event
            {
                rolling_from_row[kart] = boxes.contains(&item_box);
            }
        }
        for (kart, readout) in state.kart_readouts().iter().enumerate() {
            let landed = !matches!(readout.item_slot, ItemSlot::Rolling { .. });
            if rolling_from_row[kart] && landed {
                rolling_from_row[kart] = false;
                found.push(state.standings()[kart].spline_progress);
            }
        }
    }
    found
}

fn assert_lands_before_the_next_corner(course: fn() -> Track, row_progress: f32) {
    let track = course();
    let (boxes, corner) = row_and_next_corner(&track, row_progress);
    for level in [BotLevel::Basic, BotLevel::Ace] {
        let landed = landings(course, level, &boxes);
        assert!(
            landed.len() >= 4,
            "{level:?}: {} bots rolled from the row",
            landed.len()
        );
        for at in landed {
            assert!(
                (row_progress..corner).contains(&at),
                "{level:?}: landed at {at} m, the corner starts at {corner} m"
            );
        }
    }
}

#[test]
fn a_bot_that_rolls_from_goldstones_stowe_exit_row_holds_its_item_before_vale() {
    // The row 15 m into the run from Stowe to Vale.
    assert_lands_before_the_next_corner(goldstone, 1109.4);
}

#[test]
fn a_bot_that_rolls_from_bell_deers_back_straight_row_holds_its_item_before_130r() {
    // The row 80 m along the back straight.
    assert_lands_before_the_next_corner(bell_deer, 996.6);
}
