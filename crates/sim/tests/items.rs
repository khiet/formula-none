//! Items at the sim's public seam: a kart on an item box picks one up, the
//! box comes back the tuned time later, a kart holding an item takes boxes
//! without rolling and so denies them to the karts behind, the roll is
//! weighted by race position, a fired boost gives the tuned speed for the
//! tuned time, and a bot fires what it holds within the tuned delay.
//! Pickups and fires are read from `State::events` after the `step` that
//! produced them.

mod common;

use common::{
    oval, oval_with_boxes_under, slot_position, tap, throttle, ticks, track, with_boxes_at,
    with_boxes_under, without_coins, without_item_boxes, DT, OVAL, SEED,
};

use sim::tuning::{
    self, ITEM_BOOST_FACTOR, ITEM_BOOST_S, ITEM_BOX_HALF_LENGTH_M, ITEM_BOX_HALF_WIDTH_M,
    ITEM_BOX_RESPAWN_S, ITEM_ROLL_WEIGHTS, TOP_SPEED_M_PER_S,
};
use sim::{step, Item, KartInput, MiniTurbo, Phase, RaceEvent, State, Track};

/// The box added under slot 0 by `oval_with_boxes_under`.
fn box_under_slot_0() -> usize {
    oval().item_boxes().len()
}

/// The race past its countdown, every kart idle on the grid until Go.
fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
        assert!(
            state.events().is_empty(),
            "nothing is picked up on the grid"
        );
    }
    state
}

#[test]
fn a_kart_on_a_box_picks_up_an_item_and_the_box_comes_back_the_tuned_time_later() {
    let state = started(State::new(SEED, 1, oval_with_boxes_under(0..1)));
    assert_eq!(
        state.kart_readouts()[0].held_item,
        None,
        "empty slot on the grid"
    );

    // The first racing tick: the kart stands on the box and picks it up.
    let state = step(state, &[KartInput::default()], DT);
    let [RaceEvent::ItemPicked {
        kart: 0,
        item: Some(item),
        item_box,
    }] = state.events()
    else {
        panic!("one pickup: {:?}", state.events());
    };
    let item = *item;
    assert_eq!(*item_box, box_under_slot_0());
    assert_eq!(state.kart_readouts()[0].held_item, Some(item));

    // Fired, the slot is empty again and the box is still gone.
    let mut state = step(state, &[tap()], DT);
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired { kart: 0, item }],
        "the fire is reported, and nothing is picked up from the empty box"
    );
    assert_eq!(state.kart_readouts()[0].held_item, None);

    // The box comes back the tuned time after it was picked up, and the kart
    // still standing on it takes it again.
    let mut since_pickup = 1;
    let picked_again_after = loop {
        state = step(state, &[KartInput::default()], DT);
        since_pickup += 1;
        if let [RaceEvent::ItemPicked { kart: 0, .. }] = state.events() {
            break since_pickup;
        }
        assert!(
            state.events().is_empty(),
            "nothing else happens: {:?}",
            state.events()
        );
        assert!(
            since_pickup <= ticks(ITEM_BOX_RESPAWN_S) + 2,
            "the box never came back"
        );
    };
    let expected = ticks(ITEM_BOX_RESPAWN_S);
    assert!(
        (expected - 1..=expected + 1).contains(&picked_again_after),
        "the box came back after {picked_again_after} ticks, expected about {expected}"
    );
}

/// The oval with no coins and no item boxes but one under spawn slot 0 and
/// one 20 m down the straight ahead of it, on the line a kart driving
/// straight from slot 0 or the slot behind it takes.
fn oval_with_a_box_ahead() -> (Track, [f32; 3]) {
    let [x, y, z] = slot_position(OVAL, 0);
    let ahead = [x, y, z - 20.0];
    let sidecar = with_boxes_under(&without_coins(&without_item_boxes(OVAL)), [0]);
    (track(&with_boxes_at(&sidecar, [ahead])), ahead)
}

/// The kart, item and box of each pickup the last tick reported.
fn pickups(state: &State) -> Vec<(usize, Option<Item>, usize)> {
    state
        .events()
        .iter()
        .filter_map(|event| match *event {
            RaceEvent::ItemPicked {
                kart,
                item,
                item_box,
            } => Some((kart, item, item_box)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_kart_holding_an_item_takes_a_box_from_the_kart_behind_it_and_keeps_its_item() {
    // Kart 0 rolls an item at Go from the box under it, then drives down the
    // straight over the box ahead; the kart on the grid slot straight behind
    // it follows, its slot empty. The third kart, beside them, stays put.
    const BEHIND: usize = 2;
    const AHEAD_BOX: usize = 1;
    let (track, ahead) = oval_with_a_box_ahead();
    let mut state = started(State::new(SEED, 3, track));
    let inputs = [throttle(), KartInput::default(), throttle()];
    state = step(state, &inputs, DT);
    let [(0, Some(held), 0)] = pickups(&state)[..] else {
        panic!("kart 0 rolls at Go: {:?}", state.events());
    };

    let mut taken = Vec::new();
    let mut since_taken = 0;
    // The hitbox restated, so the test cannot pass with the kart behind off
    // the box's line.
    let mut behind_crossed_it = false;
    while state.kart_readouts()[BEHIND].position[2] > ahead[2] - 2.0 * ITEM_BOX_HALF_LENGTH_M {
        state = step(state, &inputs, DT);
        let [bx, _, bz] = state.kart_readouts()[BEHIND].position;
        behind_crossed_it |= (bx - ahead[0]).abs() < ITEM_BOX_HALF_WIDTH_M
            && (bz - ahead[2]).abs() < ITEM_BOX_HALF_LENGTH_M;
        if !taken.is_empty() {
            since_taken += 1;
        }
        taken.extend(
            pickups(&state)
                .into_iter()
                .filter(|&(_, _, b)| b == AHEAD_BOX),
        );
        assert!(
            !state.item_boxes_present()[AHEAD_BOX] || taken.is_empty(),
            "the box came back before the kart behind was past it"
        );
    }

    assert!(
        behind_crossed_it,
        "the kart behind drove over the box's spot"
    );
    assert_eq!(
        taken,
        [(0, None, AHEAD_BOX)],
        "the holding kart took it with nothing rolled, not the one behind"
    );
    assert_eq!(state.kart_readouts()[0].held_item, Some(held));
    assert_eq!(state.kart_readouts()[BEHIND].held_item, None);

    // Back the tuned time after the holding kart took it.
    while !state.item_boxes_present()[AHEAD_BOX] {
        state = step(state, &inputs, DT);
        since_taken += 1;
        assert!(
            since_taken <= ticks(ITEM_BOX_RESPAWN_S) + 2,
            "the box never came back"
        );
    }
    let expected = ticks(ITEM_BOX_RESPAWN_S);
    assert!(
        (expected - 1..=expected + 1).contains(&since_taken),
        "the box came back after {since_taken} ticks, expected about {expected}"
    );
}

#[test]
fn a_kart_that_fires_on_the_tick_its_box_comes_back_rolls_from_it() {
    // An idle kart holding an item takes its box each time it comes back,
    // rolling nothing; the sim is deterministic, so a first run finds the
    // tick the box is back and a second fires on it.
    let race = || {
        let state = started(State::new(SEED, 1, oval_with_boxes_under(0..1)));
        step(state, &[KartInput::default()], DT)
    };
    let mut state = race();
    let [(0, Some(held), _)] = pickups(&state)[..] else {
        panic!("one pickup at Go: {:?}", state.events());
    };
    let mut back_after = 0;
    loop {
        state = step(state, &[KartInput::default()], DT);
        back_after += 1;
        if !state.events().is_empty() {
            break;
        }
        assert!(
            back_after <= ticks(ITEM_BOX_RESPAWN_S) + 2,
            "the box never came back"
        );
    }
    assert_eq!(
        pickups(&state),
        [(0, None, box_under_slot_0())],
        "holding an item, the kart takes the box and rolls nothing"
    );
    assert_eq!(state.kart_readouts()[0].held_item, Some(held));
    assert!(!state.item_boxes_present()[box_under_slot_0()]);

    // Fired on that tick, the slot is empty, and the kart rolls an item from
    // the box under it.
    let mut state = race();
    for _ in 1..back_after {
        state = step(state, &[KartInput::default()], DT);
        assert!(state.events().is_empty());
    }
    let state = step(state, &[tap()], DT);
    let [RaceEvent::ItemFired { kart: 0, item }, RaceEvent::ItemPicked {
        kart: 0,
        item: Some(_),
        ..
    }] = state.events()
    else {
        panic!("a fire then a roll: {:?}", state.events());
    };
    assert_eq!(*item, held);
    assert!(state.kart_readouts()[0].held_item.is_some());
}

/// Seeds rolled per race for the weights test; eight rolls per seed.
const ROLL_SEEDS: u64 = 60;

#[test]
fn the_roll_is_weighted_by_the_race_position_band_over_many_seeds() {
    // Eight idle humans, each on a box under its slot, pick one up each at
    // Go, so one race rolls once in every race position.
    let karts = tuning::KARTS_PER_RACE;
    let mut rolls: Vec<[u32; 3]> = vec![[0; 3]; karts];
    for seed in 0..ROLL_SEEDS {
        let state = started(State::new(seed, karts, oval_with_boxes_under(0..karts)));
        let standings = state.standings();
        let state = step(state, &[KartInput::default(); 8], DT);
        assert_eq!(state.events().len(), karts, "{:?}", state.events());
        for event in state.events() {
            let RaceEvent::ItemPicked { kart, item, .. } = *event else {
                panic!("{event:?}");
            };
            assert_eq!(
                standings[kart].position as usize,
                kart + 1,
                "idle on the grid, race position is grid order"
            );
            let counts = &mut rolls[kart];
            match item.expect("an empty slot rolls") {
                Item::Hazard => counts[0] += 1,
                Item::Boost => counts[1] += 1,
                Item::Homing => counts[2] += 1,
            }
        }
    }

    // Each band's share of each item is near its weight; a band's
    // positions are pooled, since the weights are the band's.
    for band in ITEM_ROLL_WEIGHTS {
        let (first, last) = band.positions;
        let mut pooled = [0; 3];
        for position in first..=last {
            for (total, count) in pooled.iter_mut().zip(rolls[usize::from(position) - 1]) {
                *total += count;
            }
        }
        let total: u32 = pooled.iter().sum();
        assert_eq!(total, ROLL_SEEDS as u32 * u32::from(last - first + 1));
        for (name, count, weight) in [
            ("hazard", pooled[0], band.hazard),
            ("boost", pooled[1], band.boost),
            ("homing", pooled[2], band.homing),
        ] {
            let percent = count as f32 * 100.0 / total as f32;
            assert!(
                (percent - f32::from(weight)).abs() <= 10.0,
                "positions {first}-{last} rolled {name} {percent:.0}% of {total}, weight {weight}%"
            );
        }
    }
}

/// A one-kart race on a box with `item` in the kart's slot from the first
/// racing tick: the seeds are tried in order until one rolls it.
fn holding(item: Item) -> State {
    for seed in 0.. {
        let state = started(State::new(seed, 1, oval_with_boxes_under(0..1)));
        let state = step(state, &[KartInput::default()], DT);
        if state.events()
            == [RaceEvent::ItemPicked {
                kart: 0,
                item: Some(item),
                item_box: box_under_slot_0(),
            }]
        {
            return state;
        }
    }
    unreachable!()
}

#[test]
fn a_fired_boost_gives_the_tuned_speed_for_the_tuned_time() {
    let throttle = KartInput {
        throttle: 1.0,
        ..KartInput::default()
    };
    let mut state = holding(Item::Boost);
    for _ in 0..ticks(1.0) {
        state = step(state, &[throttle], DT);
    }
    let before = state.kart_readouts()[0].speed;
    assert!(
        before < TOP_SPEED_M_PER_S,
        "still accelerating at {before} m/s"
    );

    // The tick of the press is already at the boosted speed, which holds
    // for the tuned time and no longer.
    let boosted = ITEM_BOOST_FACTOR * TOP_SPEED_M_PER_S;
    let mut state = step(
        state,
        &[KartInput {
            use_item: true,
            ..throttle
        }],
        DT,
    );
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: 0,
            item: Item::Boost
        }]
    );
    assert_eq!(state.kart_readouts()[0].held_item, None);
    assert_eq!(state.kart_readouts()[0].boost_tier, MiniTurbo::Red);
    let mut boosted_ticks = 0;
    while (state.kart_readouts()[0].speed - boosted).abs() < 1e-3 {
        boosted_ticks += 1;
        state = step(state, &[throttle], DT);
        assert!(
            boosted_ticks <= ticks(ITEM_BOOST_S) + 2,
            "the boost never ended"
        );
    }
    let expected = ticks(ITEM_BOOST_S);
    assert!(
        (expected - 1..=expected + 1).contains(&boosted_ticks),
        "boosted for {boosted_ticks} ticks, expected about {expected}"
    );
    assert!(state.kart_readouts()[0].speed <= TOP_SPEED_M_PER_S);
}

#[test]
fn a_fired_hazard_or_homing_projectile_leaves_the_slot_empty_and_the_kart_unboosted() {
    for item in [Item::Hazard, Item::Homing] {
        let state = step(holding(item), &[tap()], DT);
        assert_eq!(state.events(), &[RaceEvent::ItemFired { kart: 0, item }]);
        assert_eq!(state.kart_readouts()[0].held_item, None);
        assert!(state.kart_readouts()[0].speed <= TOP_SPEED_M_PER_S);
    }
}

#[test]
fn a_bot_fires_what_it_picks_up_within_the_tuned_delay() {
    use sim::tuning::BOT_ITEM_USE_DELAY_S;
    // A lone bot laps the oval; its line runs through the rows of boxes on
    // the back straight.
    let mut state = started(State::with_bots(SEED, 0, 1, oval()));
    let mut picked: Option<(usize, Item)> = None;
    let mut fired = Vec::new();
    for tick in 0..ticks(60.0) {
        state = step(state, &[], DT);
        for event in state.events() {
            match *event {
                RaceEvent::ItemPicked {
                    kart: 0,
                    item: Some(item),
                    ..
                } => {
                    assert_eq!(picked, None, "one item at a time");
                    picked = Some((tick, item));
                }
                RaceEvent::ItemFired { kart: 0, item } => {
                    let (picked_at, held) = picked.take().expect("fired what it picked up");
                    assert_eq!(item, held);
                    assert!(
                        tick - picked_at <= ticks(BOT_ITEM_USE_DELAY_S),
                        "fired {} s after the pickup",
                        (tick - picked_at) as f32 * DT
                    );
                    fired.push(item);
                }
                _ => {}
            }
        }
        if fired.len() >= 2 {
            return;
        }
    }
    panic!("the bot fired {fired:?} in 60 s");
}

#[test]
fn the_item_button_held_down_fires_once_as_it_goes_down() {
    let hold = KartInput {
        hold_item: true,
        ..KartInput::default()
    };
    let state = holding(Item::Hazard);
    let state = step(state, &[hold], DT);
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: 0,
            item: Item::Hazard
        }],
        "the button going down fires without a press being reported"
    );
    // Held through the box coming back under the kart, the button fires
    // nothing more; let go and pressed again, it fires the new item.
    let mut state = state;
    for _ in 0..ticks(ITEM_BOX_RESPAWN_S) + 2 {
        state = step(state, &[hold], DT);
        assert!(
            !state
                .events()
                .iter()
                .any(|event| matches!(event, RaceEvent::ItemFired { .. })),
            "{:?}",
            state.events()
        );
    }
    assert!(state.kart_readouts()[0].held_item.is_some());
    let state = step(state, &[KartInput::default()], DT);
    let state = step(state, &[hold], DT);
    assert!(
        matches!(state.events(), [RaceEvent::ItemFired { kart: 0, .. }, ..]),
        "{:?}",
        state.events()
    );
}
