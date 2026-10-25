//! Items at the sim's public seam: a kart on an item box picks one up, the
//! box comes back the tuned time later, a kart holding an item picks up
//! nothing more, the roll is weighted by race position, a fired boost gives
//! the tuned speed for the tuned time, and a bot fires what it holds within
//! the tuned delay. Pickups and fires are read from `State::events` after
//! the `step` that produced them.

use sim::tuning::{
    self, ITEM_BOOST_FACTOR, ITEM_BOOST_S, ITEM_BOX_RESPAWN_S, ITEM_ROLL_WEIGHTS, TICKS_PER_SECOND,
    TOP_SPEED_M_PER_S,
};
use sim::{step, Item, KartInput, Phase, RaceEvent, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

/// The oval with one more item box, under each of the first `slots` spawn
/// slots, so a kart there stands on a box at Go. The added boxes come after
/// the fixture's own, in slot order.
fn oval_with_boxes_under_slots(slots: usize) -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    for slot in 0..slots {
        let position = sidecar["spawn_grid"][slot]["position"].clone();
        sidecar["item_boxes"]
            .as_array_mut()
            .expect("the fixture lists its boxes")
            .push(serde_json::json!({ "position": position, "yaw": 0.0 }));
    }
    Track::from_sidecar(&sidecar.to_string()).expect("the oval with more boxes loads")
}

/// The box added under slot 0 by `oval_with_boxes_under_slots`.
fn box_under_slot_0() -> usize {
    Track::from_sidecar(OVAL)
        .expect("the oval fixture loads")
        .item_boxes()
        .len()
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

fn fire() -> KartInput {
    KartInput {
        use_item: true,
        ..KartInput::default()
    }
}

#[test]
fn a_kart_on_a_box_picks_up_an_item_and_the_box_comes_back_the_tuned_time_later() {
    let state = started(State::new(SEED, 1, oval_with_boxes_under_slots(1)));
    assert_eq!(
        state.kart_readouts()[0].held_item,
        None,
        "empty slot on the grid"
    );

    // The first racing tick: the kart stands on the box and picks it up.
    let state = step(state, &[KartInput::default()], DT);
    let [RaceEvent::ItemPicked {
        kart: 0,
        item,
        item_box,
    }] = state.events()
    else {
        panic!("one pickup: {:?}", state.events());
    };
    let item = *item;
    assert_eq!(*item_box, box_under_slot_0());
    assert_eq!(state.kart_readouts()[0].held_item, Some(item));

    // Fired, the slot is empty again and the box is still gone.
    let mut state = step(state, &[fire()], DT);
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

#[test]
fn a_kart_holding_an_item_takes_no_box_until_it_fires() {
    let state = started(State::new(SEED, 1, oval_with_boxes_under_slots(1)));
    let mut state = step(state, &[KartInput::default()], DT);
    let [RaceEvent::ItemPicked { item, .. }] = state.events() else {
        panic!("one pickup: {:?}", state.events());
    };
    let held = *item;

    // The box comes back under the kart, which holds on to its item and
    // leaves the box where it is.
    for _ in 0..ticks(2.0 * ITEM_BOX_RESPAWN_S) {
        state = step(state, &[KartInput::default()], DT);
        assert!(
            state.events().is_empty(),
            "a full slot takes nothing: {:?}",
            state.events()
        );
    }
    assert_eq!(state.kart_readouts()[0].held_item, Some(held));
    assert!(
        state.item_boxes_present()[box_under_slot_0()],
        "the box is back and untaken"
    );

    // Fired, the slot is empty, and the box under the kart is picked up the
    // same tick.
    let state = step(state, &[fire()], DT);
    let [RaceEvent::ItemFired { kart: 0, item }, RaceEvent::ItemPicked { kart: 0, .. }] =
        state.events()
    else {
        panic!("a fire then a pickup: {:?}", state.events());
    };
    assert_eq!(*item, held);
    assert!(state.kart_readouts()[0].held_item.is_some());
    assert!(!state.item_boxes_present()[box_under_slot_0()]);
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
        let state = started(State::new(seed, karts, oval_with_boxes_under_slots(karts)));
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
            match item {
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
        let state = started(State::new(seed, 1, oval_with_boxes_under_slots(1)));
        let state = step(state, &[KartInput::default()], DT);
        if state.events()
            == [RaceEvent::ItemPicked {
                kart: 0,
                item,
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
        let state = step(holding(item), &[fire()], DT);
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
    let mut state = started(State::with_bots(
        SEED,
        0,
        1,
        Track::from_sidecar(OVAL).unwrap(),
    ));
    let mut picked: Option<(usize, Item)> = None;
    let mut fired = Vec::new();
    for tick in 0..ticks(60.0) {
        state = step(state, &[], DT);
        for event in state.events() {
            match *event {
                RaceEvent::ItemPicked { kart: 0, item, .. } => {
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
