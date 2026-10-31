//! Coins at the sim's public seam: a kart whose centre is on a coin takes
//! it, its count holding at a full purse, and the coin is back the tuned
//! time later, so a full kart denies it to the kart behind; each
//! coin raises the kart's forward top speed, with a boost on top; every
//! spin-out, from a hazard, a homing projectile or the runaway wheel, costs
//! the tuned three or all the kart has; and a spun-out kart takes nothing.
//! Pickups and losses are read from `State::events` after the `step` that
//! produced them, the count from the kart readout.

mod common;

use common::{
    bell_deer, slot_position, started, tap, throttle, ticks, track, with_boxes_under,
    with_coins_at, with_slot_at, without_coins, without_item_boxes, without_walls, BELL_DEER, DT,
    OVAL, SEED,
};

use std::f32::consts::FRAC_PI_2;

use sim::tuning::{
    BOOST_PAD_FACTOR, COINS_LOST_PER_SPIN_OUT, COIN_CAP, COIN_HALF_LENGTH_M, COIN_RESPAWN_S,
    OFF_TRACK_SPEED_FACTOR, TUNING,
};
use sim::{step, Item, KartInput, Phase, RaceEvent, State, Surface, Track};

/// Metres between the coins of a line laid by `line_ahead_of`: wider than
/// a coin, so a kart takes them one at a time.
const LINE_SPACING_M: f32 = 3.0;

/// The oval with no coins but those at `positions` and no item boxes but
/// one under each of `box_slots`.
fn oval_with(positions: impl IntoIterator<Item = [f32; 3]>, box_slots: &[usize]) -> Track {
    let sidecar = without_coins(&without_item_boxes(OVAL));
    let sidecar = with_boxes_under(&sidecar, box_slots.iter().copied());
    track(&with_coins_at(&sidecar, positions))
}

/// `count` coins stacked under the oval's spawn slot `slot`: a kart there
/// takes them all, up to a full purse, at Go.
fn under(slot: usize, count: usize) -> Vec<[f32; 3]> {
    vec![slot_position(OVAL, slot); count]
}

/// A line of `count` coins down the oval's front straight from under spawn
/// slot `slot`, on the line a kart driving straight from it takes.
fn line_ahead_of(slot: usize, count: usize) -> Vec<[f32; 3]> {
    let [x, y, z] = slot_position(OVAL, slot);
    (0..count)
        .map(|k| [x, y, z - LINE_SPACING_M * k as f32])
        .collect()
}

fn coins(state: &State, kart: usize) -> u32 {
    state.kart_readouts()[kart].coins
}

fn pickups(state: &State) -> Vec<RaceEvent> {
    state
        .events()
        .iter()
        .copied()
        .filter(|event| matches!(event, RaceEvent::CoinPicked { .. }))
        .collect()
}

fn losses(state: &State) -> Vec<RaceEvent> {
    state
        .events()
        .iter()
        .copied()
        .filter(|event| matches!(event, RaceEvent::CoinsLost { .. }))
        .collect()
}

fn idle() -> KartInput {
    KartInput::default()
}

/// The top speed of a kart carrying `count` coins, as the sim's tuning
/// hands it to the Instrument panel.
fn top_speed_with(count: u32) -> f32 {
    TUNING.coin_top_speeds_m_per_s[count as usize]
}

#[test]
fn every_kart_starts_the_race_with_no_coins_and_takes_none_through_the_countdown() {
    let positions = (0..4).flat_map(|slot| under(slot, 2));
    let mut state = State::new(SEED, 4, oval_with(positions, &[]));
    while matches!(state.phase(), Phase::Countdown { .. }) {
        assert!(state.kart_readouts().iter().all(|kart| kart.coins == 0));
        state = step(state, &[idle(); 4], DT);
        assert!(pickups(&state).is_empty(), "nothing is taken on the grid");
    }
    assert!(state.kart_readouts().iter().all(|kart| kart.coins == 0));
}

#[test]
fn a_kart_on_a_coin_takes_it_and_the_coin_is_back_the_tuned_time_later() {
    let state = started(State::new(SEED, 1, oval_with(under(0, 1), &[])));
    assert_eq!(state.coins_present(), vec![true]);

    // The first racing tick: the kart stands on the coin and takes it.
    let mut state = step(state, &[idle()], DT);
    assert_eq!(
        pickups(&state),
        [RaceEvent::CoinPicked { kart: 0, coin: 0 }]
    );
    assert_eq!(coins(&state, 0), 1);
    assert_eq!(state.coins_present(), vec![false]);

    // Back the tuned time later, and taken again by the kart still on it.
    let mut since_pickup = 0;
    loop {
        state = step(state, &[idle()], DT);
        since_pickup += 1;
        if !pickups(&state).is_empty() {
            break;
        }
        assert!(
            since_pickup <= ticks(COIN_RESPAWN_S) + 2,
            "the coin never came back"
        );
    }
    let expected = ticks(COIN_RESPAWN_S);
    assert!(
        (expected - 1..=expected + 1).contains(&since_pickup),
        "the coin came back after {since_pickup} ticks, expected about {expected}"
    );
    assert_eq!(coins(&state, 0), 2);
}

#[test]
fn a_kart_takes_a_line_of_coins_one_at_a_time_up_to_a_full_purse() {
    let purse = COIN_CAP as usize;
    let line = line_ahead_of(0, purse + 1);
    let last = line[purse];
    let mut state = started(State::new(SEED, 1, oval_with(line, &[])));
    let mut taken = Vec::new();
    // Past the last coin, before the first is back.
    while state.kart_readouts()[0].position[2] > last[2] - 2.0 * COIN_HALF_LENGTH_M {
        state = step(state, &[throttle()], DT);
        for event in pickups(&state) {
            let RaceEvent::CoinPicked { coin, .. } = event else {
                unreachable!()
            };
            taken.push(coin);
        }
        assert!(
            !state.coins_present()[0],
            "the first coin came back before the kart was past the line"
        );
    }

    assert_eq!(
        taken,
        (0..=purse).collect::<Vec<_>>(),
        "one at a time, in order, the last with the purse already full"
    );
    assert_eq!(
        coins(&state, 0),
        COIN_CAP,
        "the count stops at a full purse"
    );
    assert!(!state.coins_present()[purse], "the last coin is taken too");
}

#[test]
fn a_kart_carrying_a_full_purse_takes_a_coin_from_the_kart_behind_it() {
    // Kart 0 fills its purse at Go from the stack under it; kart 2, on the
    // slot straight behind, follows it down the straight over the coin ahead.
    let purse = COIN_CAP as usize;
    let [x, y, z] = slot_position(OVAL, 0);
    let ahead = [x, y, z - 20.0];
    let positions = under(0, purse).into_iter().chain([ahead]);
    let mut state = started(State::new(SEED, 3, oval_with(positions, &[])));
    let inputs = [throttle(), idle(), throttle()];
    state = step(state, &inputs, DT);
    assert_eq!(coins(&state, 0), COIN_CAP);

    let mut taken_by = Vec::new();
    while state.kart_readouts()[2].position[2] > ahead[2] - 2.0 * COIN_HALF_LENGTH_M {
        state = step(state, &inputs, DT);
        taken_by.extend(pickups(&state).into_iter().map(|event| {
            let RaceEvent::CoinPicked { kart, coin } = event else {
                unreachable!()
            };
            (kart, coin)
        }));
        assert!(
            !state.coins_present()[purse] || taken_by.is_empty(),
            "the coin came back before the kart behind was past it"
        );
    }

    assert_eq!(
        taken_by,
        [(0, purse)],
        "the full kart took it, not the one behind"
    );
    assert_eq!(coins(&state, 0), COIN_CAP);
    assert_eq!(coins(&state, 2), 0);
}

#[test]
fn a_full_purse_raises_top_speed_by_the_tuned_factor_per_coin() {
    let mut state = started(State::new(SEED, 1, oval_with(line_ahead_of(0, 10), &[])));
    for _ in 0..ticks(4.0) {
        state = step(state, &[throttle()], DT);
    }

    assert_eq!(coins(&state, 0), COIN_CAP);
    let speed = state.kart_readouts()[0].speed;
    assert!(
        (speed - top_speed_with(COIN_CAP)).abs() < 1e-3,
        "{speed} m/s, expected {}",
        top_speed_with(COIN_CAP)
    );
    assert!((top_speed_with(COIN_CAP) - 33.0).abs() < 1e-3);
}

#[test]
fn a_boost_multiplies_on_top_of_the_coins() {
    // From slot 1's spot the kart crosses the front straight's pad, a full
    // purse on.
    let sidecar = with_slot_at(&without_coins(OVAL), 0, slot_position(OVAL, 1), 0.0);
    let sidecar = with_coins_at(&sidecar, line_ahead_of(1, 10));
    let mut state = started(State::new(SEED, 1, track(&sidecar)));
    let boosted = loop {
        state = step(state, &[throttle()], DT);
        let kart = state.kart_readouts()[0];
        if kart.boost_s.is_some() {
            break kart;
        }
        assert!(
            matches!(state.phase(), Phase::Racing { elapsed_s, .. } if elapsed_s < 8.0),
            "never reached the pad"
        );
    };

    assert_eq!(boosted.coins, COIN_CAP);
    let expected = top_speed_with(COIN_CAP) * BOOST_PAD_FACTOR;
    assert!(
        (boosted.speed - expected).abs() < 1e-3,
        "{} m/s, expected {expected}",
        boosted.speed
    );
}

#[test]
fn off_the_asphalt_the_cap_is_raised_by_the_coins_too() {
    // On the oval's infield grass facing +X, walls gone, a full purse stacked
    // under the kart: the grass runs to x = 18.
    let on_grass = [0.0, 0.0, -20.0];
    let sidecar = with_slot_at(
        &without_walls(&without_coins(OVAL)),
        0,
        on_grass,
        -FRAC_PI_2,
    );
    let sidecar = with_coins_at(&sidecar, vec![on_grass; COIN_CAP as usize]);
    let mut state = started(State::new(SEED, 1, track(&sidecar)));
    let mut fastest: f32 = 0.0;
    while state.kart_readouts()[0].position[0] < 17.0 {
        assert_eq!(state.kart_surface(0), Surface::Grass);
        state = step(state, &[throttle()], DT);
        fastest = fastest.max(state.kart_readouts()[0].speed);
    }

    assert_eq!(coins(&state, 0), COIN_CAP);
    let cap = OFF_TRACK_SPEED_FACTOR * top_speed_with(COIN_CAP);
    assert!(
        (fastest - cap).abs() < 1e-3,
        "{fastest} m/s, expected {cap}"
    );
}

/// A race of four karts one tick past Go: kart 1 holds a hazard from a box
/// under its slot, and kart 3, behind it, has taken `count` coins stacked
/// under its own. The seeds are tried in order until kart 1 rolls the
/// hazard.
fn kart_3_behind_a_hazard_with(count: usize) -> State {
    for seed in 0.. {
        let track = oval_with(under(3, count), &[1]);
        let state = step(started(State::new(seed, 4, track)), &[idle(); 4], DT);
        let rolled = state.events().contains(&RaceEvent::ItemPicked {
            kart: 1,
            item: Item::Hazard,
            item_box: 0,
        });
        if rolled {
            return state;
        }
    }
    unreachable!()
}

/// Kart 1 drops its hazard and drives off, kart 3 drives into it: the
/// state the tick kart 3 is spun out.
fn hazard_hits_kart_3(state: State) -> State {
    let mut state = step(state, &[idle(), tap(), idle(), throttle()], DT);
    for _ in 0..ticks(3.0) {
        state = step(state, &[idle(), throttle(), idle(), throttle()], DT);
        let hit = RaceEvent::SpunOut {
            kart: 3,
            by: Some(1),
        };
        if state.events().contains(&hit) {
            return state;
        }
    }
    panic!("kart 3 never reached the hazard");
}

#[test]
fn a_hazard_spin_out_costs_the_tuned_three_coins_which_vanish() {
    let state = kart_3_behind_a_hazard_with(5);
    assert_eq!(coins(&state, 3), 5);

    let state = hazard_hits_kart_3(state);

    assert_eq!(
        losses(&state),
        [RaceEvent::CoinsLost {
            kart: 3,
            count: COINS_LOST_PER_SPIN_OUT
        }]
    );
    assert_eq!(coins(&state, 3), 5 - COINS_LOST_PER_SPIN_OUT);
    assert_eq!(state.coins_present().len(), 5, "nothing is scattered");
}

#[test]
fn a_kart_with_fewer_than_three_loses_what_it_has() {
    let state = hazard_hits_kart_3(kart_3_behind_a_hazard_with(2));

    assert_eq!(losses(&state), [RaceEvent::CoinsLost { kart: 3, count: 2 }]);
    assert_eq!(coins(&state, 3), 0);
}

#[test]
fn a_kart_with_no_coins_is_spun_out_without_a_loss() {
    let state = hazard_hits_kart_3(kart_3_behind_a_hazard_with(0));

    assert!(losses(&state).is_empty());
    assert_eq!(coins(&state, 3), 0);
}

/// A race of two karts one tick past Go: kart 1 holds a homing projectile
/// from a box under its slot and kart 0, ahead of it, has taken `count`
/// coins stacked under its own. The seeds are tried in order until kart 1
/// rolls the projectile.
fn kart_1_homing_on_kart_0_with(count: usize) -> State {
    for seed in 0.. {
        let track = oval_with(under(0, count), &[1]);
        let state = step(started(State::new(seed, 2, track)), &[idle(); 2], DT);
        let rolled = state.events().contains(&RaceEvent::ItemPicked {
            kart: 1,
            item: Item::Homing,
            item_box: 0,
        });
        if rolled {
            return state;
        }
    }
    unreachable!()
}

/// Steps until kart 0 is spun out by kart 1's projectile, both idle.
fn until_kart_0_is_hit(mut state: State) -> State {
    for _ in 0..ticks(1.0) {
        state = step(state, &[idle(); 2], DT);
        let hit = RaceEvent::SpunOut {
            kart: 0,
            by: Some(1),
        };
        if state.events().contains(&hit) {
            return state;
        }
    }
    panic!("the projectile never reached kart 0");
}

#[test]
fn a_homing_projectile_spin_out_costs_the_tuned_three_coins() {
    let state = kart_1_homing_on_kart_0_with(4);
    assert_eq!(coins(&state, 0), 4);

    let state = until_kart_0_is_hit(step(state, &[idle(), tap()], DT));

    assert_eq!(
        losses(&state),
        [RaceEvent::CoinsLost {
            kart: 0,
            count: COINS_LOST_PER_SPIN_OUT
        }]
    );
    assert_eq!(coins(&state, 0), 4 - COINS_LOST_PER_SPIN_OUT);
}

#[test]
fn a_spun_out_kart_takes_no_coin_until_its_spin_out_is_over() {
    let mut state = kart_1_homing_on_kart_0_with(1);
    // Fired as the taken coin is about to come back, so it is back under
    // kart 0 while the spin-out holds it.
    for _ in 0..ticks(COIN_RESPAWN_S - 0.5) {
        state = step(state, &[idle(); 2], DT);
    }
    let mut state = until_kart_0_is_hit(step(state, &[idle(), tap()], DT));
    assert_eq!(coins(&state, 0), 0, "the one coin is lost");

    let mut back_while_spun_out = false;
    while state.kart_readouts()[0].spin_out_s.is_some() {
        back_while_spun_out |= state.coins_present()[0];
        assert!(pickups(&state).is_empty(), "nothing is taken spun out");
        assert_eq!(coins(&state, 0), 0);
        state = step(state, &[idle(); 2], DT);
    }
    assert!(
        back_while_spun_out,
        "the coin came back under the spun-out kart"
    );

    // Over, the kart takes the coin it stands on.
    let state = step(state, &[idle(); 2], DT);
    assert_eq!(
        pickups(&state),
        [RaceEvent::CoinPicked { kart: 0, coin: 0 }]
    );
    assert_eq!(coins(&state, 0), 1);
}

#[test]
fn a_runaway_wheel_spin_out_costs_the_tuned_three_coins() {
    // Parked by the crossing's left wall on its line, where the first pass
    // rolls in, on a full purse stacked under the kart.
    let crossing = bell_deer().wheel_crossings()[0];
    let (sin, cos) = crossing.yaw.sin_cos();
    let left = crossing.half_width - 1.5;
    let [x, y, z] = crossing.position;
    let parked = [x - left * cos, y, z + left * sin];
    let sidecar = with_slot_at(&without_coins(BELL_DEER), 0, parked, crossing.yaw);
    let sidecar = with_coins_at(&sidecar, vec![parked; COIN_CAP as usize]);
    let mut state = step(started(State::new(SEED, 1, track(&sidecar))), &[idle()], DT);
    assert_eq!(coins(&state, 0), COIN_CAP);

    let hit = RaceEvent::SpunOut { kart: 0, by: None };
    while !state.events().contains(&hit) {
        assert!(
            matches!(state.phase(), Phase::Racing { elapsed_s, .. } if elapsed_s < 40.0),
            "the wheel never reached the kart"
        );
        state = step(state, &[idle()], DT);
    }

    assert_eq!(
        losses(&state),
        [RaceEvent::CoinsLost {
            kart: 0,
            count: COINS_LOST_PER_SPIN_OUT
        }]
    );
    assert_eq!(coins(&state, 0), COIN_CAP - COINS_LOST_PER_SPIN_OUT);
}
