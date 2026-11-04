//! The wire's types for the sim's and back (ADR 0026): a kart readout
//! through a snapshot keeps everything the snapshot carries, and comes back
//! as the sim reports a kart only a snapshot speaks for.

use protocol::{EventKind, Input, KartSnapshot};
use sim::{
    Drift, DriftDirection, Item, ItemSlot, KartInput, KartReadout, MiniTurbo, RaceEvent,
    Slipstream, Start, Stop, StopTier,
};

/// A kart mid-race with every field the snapshot carries away from its
/// default: drifting left at red, rolling a hazard, carrying seven coins, a hop and an orange
/// boost under way, spun out, stalled, on a star's run, third on lap 2 and
/// going the wrong way.
/// Turned a quarter left, so its facing is -X and its velocity runs along
/// it at 12 m/s, rising, with a bump from a touch carrying it sideways.
fn busy_kart() -> KartReadout {
    let half = std::f32::consts::FRAC_1_SQRT_2;
    KartReadout {
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, half, 0.0, half],
        speed: 12.5,
        velocity: [-12.0, 0.5, 0.0],
        bump: Some([0.0, 0.0, 3.0]),
        drift: Some(Drift {
            direction: DriftDirection::Left,
            charge_s: 0.75,
        }),
        mini_turbo: MiniTurbo::Red,
        item_slot: ItemSlot::Rolling {
            item: Item::Hazard,
            roll_left_s: 1.5,
        },
        coins: 7,
        tyre_life: 42.5,
        spin_out_s: Some(0.4),
        airborne: true,
        in_pit_lane: false,
        stop: Some(Stop {
            tier: StopTier::Botched,
            stationary_s: 5.5,
            guide_left_s: 0.0,
            stationary_so_far_s: 2.25,
        }),
        boost_s: Some(0.2),
        boost_tier: MiniTurbo::Orange,
        start: Start::WeakBoost,
        stall_s: Some(0.1),
        star_s: Some(4.5),
        slipstream: Some(Slipstream::Tow { left_s: 0.6 }),
        race_position: 3,
        lap: 2,
        checkpoint: 7,
        spline_progress: 123.5,
        wrong_way: true,
    }
}

#[test]
fn a_readout_through_a_snapshot_keeps_what_the_snapshot_carries() {
    let kart = busy_kart();
    let back = KartReadout::from(&KartSnapshot::from(&kart));

    assert_eq!(
        back,
        KartReadout {
            speed: back.speed,
            airborne: false,
            in_pit_lane: false,
            ..kart
        }
    );
    assert!(
        (back.speed - 12.0).abs() < 1e-5,
        "the speed is the velocity along the facing, {}",
        back.speed
    );
}

#[test]
fn the_mini_turbo_tier_is_read_from_the_drift_the_snapshot_carries() {
    let orange = KartReadout {
        drift: Some(Drift {
            direction: DriftDirection::Right,
            charge_s: sim::tuning::MINI_TURBO_ORANGE_CHARGE_S,
        }),
        ..busy_kart()
    };
    assert_eq!(
        KartReadout::from(&KartSnapshot::from(&orange)).mini_turbo,
        MiniTurbo::Orange
    );
    let straight = KartReadout {
        drift: None,
        ..busy_kart()
    };
    assert_eq!(
        KartReadout::from(&KartSnapshot::from(&straight)).mini_turbo,
        MiniTurbo::None
    );
}

#[test]
fn a_boost_is_red_through_a_snapshot_unless_it_burns_orange() {
    let red = KartReadout {
        boost_tier: MiniTurbo::Red,
        ..busy_kart()
    };
    assert_eq!(
        KartReadout::from(&KartSnapshot::from(&red)).boost_tier,
        MiniTurbo::Red
    );
    let unboosted = KartReadout {
        boost_s: None,
        boost_tier: MiniTurbo::None,
        ..busy_kart()
    };
    let back = KartReadout::from(&KartSnapshot::from(&unboosted));
    assert_eq!((back.boost_s, back.boost_tier), (None, MiniTurbo::None));
}

#[test]
fn the_slipstream_crosses_a_snapshot_in_both_states_and_as_nothing() {
    for slipstream in [
        None,
        Some(Slipstream::Charging { s: 0.35 }),
        Some(Slipstream::Tow { left_s: 0.8 }),
    ] {
        let kart = KartReadout {
            slipstream,
            ..busy_kart()
        };
        assert_eq!(
            KartReadout::from(&KartSnapshot::from(&kart)).slipstream,
            slipstream
        );
    }
}

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
