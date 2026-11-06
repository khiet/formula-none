//! The wire's types for the sim's and back (ADR 0026), for what is not a
//! kart: a kart crosses as its readout (ADR 0090).

use protocol::{EventKind, Input};
use sim::{KartInput, RaceEvent};

#[test]
fn a_spin_out_is_told_as_a_hit() {
    assert_eq!(
        EventKind::from(RaceEvent::SpunOut {
            kart: 2,
            by: Some(5)
        }),
        EventKind::Hit {
            kart: 2,
            by: Some(5)
        }
    );
}

#[test]
fn a_coin_pickup_names_the_coin_and_a_loss_its_count() {
    assert_eq!(
        EventKind::from(RaceEvent::CoinPicked { kart: 4, coin: 13 }),
        EventKind::CoinPicked { kart: 4, coin: 13 }
    );
    assert_eq!(
        EventKind::from(RaceEvent::CoinsLost { kart: 4, count: 3 }),
        EventKind::CoinsLost { kart: 4, count: 3 }
    );
}

#[test]
fn an_input_reads_as_the_unit_range_controls() {
    let input = Input {
        sequence: 9,
        steer: -127,
        throttle: 255,
        brake: 0,
        hop: true,
        use_item: false,
        hold_item: true,
    };
    assert_eq!(
        KartInput::from(input),
        KartInput {
            steer: -1.0,
            throttle: 1.0,
            brake: 0.0,
            hop: true,
            use_item: false,
            hold_item: true,
        }
    );
}

#[test]
fn the_runaway_wheels_spin_out_is_told_as_a_hit_by_no_kart() {
    assert_eq!(
        EventKind::from(RaceEvent::SpunOut { kart: 2, by: None }),
        EventKind::Hit { kart: 2, by: None }
    );
}
