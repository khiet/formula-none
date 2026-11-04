//! The laps a race runs (ADR 0084): a kart finishes on the last of them, for
//! the state and for a prediction of it.

mod common;

use common::{oval, started, ticks, DT, SEED};
use sim::{step, BotLevel, Prediction, Standing, State};

/// A lone bot's standing as it finishes a race on the oval over `laps`,
/// or fails the test when it has not after ninety seconds a lap.
fn standing_at_the_finish(laps: u32) -> Standing {
    let mut state = started(State::with_bots(SEED, 0, 1, oval(), BotLevel::Basic).with_laps(laps));
    for _ in 0..ticks(laps as f32 * 90.0) {
        state = step(state, &[], DT);
        if state.standings()[0].finish_time_s.is_some() {
            break;
        }
    }
    let standing = state.standings()[0];
    assert!(standing.finish_time_s.is_some(), "{laps:?}: {standing:?}");
    standing
}

#[test]
fn a_race_of_three_laps_finishes_a_kart_on_its_third() {
    let standing = standing_at_the_finish(3);
    assert_eq!(standing.laps_completed, 3);
    assert_eq!(standing.lap, 3);
}

#[test]
fn a_race_of_five_laps_finishes_a_kart_on_its_fifth() {
    let standing = standing_at_the_finish(5);
    assert_eq!(standing.laps_completed, 5);
    assert_eq!(standing.lap, 5);
}

/// The lap a prediction of a race over `laps` holds after a snapshot puts
/// its kart on lap 4.
fn predicted_lap_after_a_snapshot_on_lap_four(laps: u32) -> u32 {
    let server = State::with_bots(SEED, 1, 0, oval(), BotLevel::Basic);
    let mut prediction = Prediction::new(SEED, 0, oval(), BotLevel::Basic).with_laps(laps);
    prediction.tick(1, Default::default(), &[]);
    let mut reading = server.kart_readouts()[0];
    reading.lap = 4;
    prediction.reconcile(1, 1, &reading);
    prediction.standing().lap
}

#[test]
fn a_prediction_of_five_laps_holds_a_lap_past_the_third() {
    assert_eq!(predicted_lap_after_a_snapshot_on_lap_four(5), 4);
    assert_eq!(predicted_lap_after_a_snapshot_on_lap_four(3), 3);
}
