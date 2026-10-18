//! The tuning module is the public home of every constant; these values are
//! the PRD's, so a change here is a tuning decision, not a refactor.

use sim::tuning;

#[test]
fn sim_ticks_sixty_times_a_second() {
    assert_eq!(tuning::TICKS_PER_SECOND, 60);
}

#[test]
fn a_race_is_three_laps_for_eight_karts() {
    assert_eq!(tuning::LAPS_PER_RACE, 3);
    assert_eq!(tuning::KARTS_PER_RACE, 8);
}
