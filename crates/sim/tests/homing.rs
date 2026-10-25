//! The homing projectile at the sim's public seam: fired, it travels the
//! spline at the tuned speed toward the kart next ahead of its owner by
//! race position, spins that kart out for the tuned time, and is destroyed
//! by a hazard the target trails, which is used up. Fired with no kart
//! ahead it is wasted. Projectiles are read from `State::projectiles` and
//! spin-outs from `State::events` after the `step` that produced them.

use sim::tuning::{
    BOT_ITEM_USE_DELAY_S, HOMING_SPEED_FACTOR, SPIN_OUT_S, TICKS_PER_SECOND, TOP_SPEED_M_PER_S,
};
use sim::{step, Item, KartInput, Phase, RaceEvent, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

/// The oval with one more item box under each of `slots` of the spawn
/// grid, so a kart there stands on a box at Go.
fn oval_with_boxes_under(slots: &[usize]) -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    for &slot in slots {
        let position = sidecar["spawn_grid"][slot]["position"].clone();
        sidecar["item_boxes"]
            .as_array_mut()
            .expect("the fixture lists its boxes")
            .push(serde_json::json!({ "position": position, "yaw": 0.0 }));
    }
    Track::from_sidecar(&sidecar.to_string()).expect("the oval with more boxes loads")
}

/// The race past its countdown, every kart idle on the grid until Go.
fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
    }
    state
}

/// A race of `humans` karts and `bots` bots one tick past Go, each of
/// `rolls` standing on a box under its slot and holding the item named,
/// with every bot rolling the same rocket start so the items decide the
/// race and not the start: the seeds are tried in order until one does.
fn holding_with_bots(humans: usize, bots: usize, rolls: &[(usize, Item)]) -> State {
    let slots: Vec<usize> = rolls.iter().map(|&(slot, _)| slot).collect();
    for seed in 0.. {
        let state = started(State::with_bots(
            seed,
            humans,
            bots,
            oval_with_boxes_under(&slots),
        ));
        let even_start = state.kart_readouts()[humans..]
            .windows(2)
            .all(|pair| pair[0].start == pair[1].start);
        let state = step(state, &vec![KartInput::default(); humans], DT);
        let rolled = rolls.iter().all(|&(slot, wanted)| {
            state.events().iter().any(|event| {
                matches!(event, RaceEvent::ItemPicked { kart, item, .. } if *kart == slot && *item == wanted)
            })
        });
        if rolled && even_start {
            return state;
        }
    }
    unreachable!()
}

/// `holding_with_bots` for a race of humans only.
fn holding(humans: usize, rolls: &[(usize, Item)]) -> State {
    holding_with_bots(humans, 0, rolls)
}

fn drive() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

fn idle() -> KartInput {
    KartInput::default()
}

/// A press and release in one tick.
fn tap() -> KartInput {
    KartInput {
        use_item: true,
        ..KartInput::default()
    }
}

fn spun_out(events: &[RaceEvent]) -> Option<RaceEvent> {
    events
        .iter()
        .copied()
        .find(|event| matches!(event, RaceEvent::SpunOut { .. }))
}

/// Ground distance between two points, in metres.
fn ground_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn a_fired_projectile_travels_the_spline_at_the_tuned_speed_and_spins_out_the_kart_ahead() {
    // Kart 0 drives off from pole; kart 1, beside it on the grid, fires
    // after it two seconds later.
    let mut state = holding(2, &[(1, Item::Homing)]);
    for _ in 0..ticks(2.0) {
        state = step(state, &[drive(), idle()], DT);
    }
    assert!(state.projectiles().is_empty(), "nothing fired yet");
    let fired_from = state.kart_readouts()[1].position;

    let state = step(state, &[drive(), tap()], DT);
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: 1,
            item: Item::Homing
        }]
    );
    assert_eq!(state.kart_readouts()[1].held_item, None);
    let [projectile] = state.projectiles()[..] else {
        panic!("one projectile: {:?}", state.projectiles());
    };
    assert_eq!((projectile.owner, projectile.target), (1, 0));
    assert!(
        ground_distance(projectile.position, fired_from) < 1.0,
        "spawned at the kart: {:?} from {fired_from:?}",
        projectile.position
    );

    // Every tick it flies the tuned speed, along the spline, until it
    // reaches kart 0.
    let track = state.track().clone();
    let speed = HOMING_SPEED_FACTOR * TOP_SPEED_M_PER_S;
    let mut state = state;
    let mut last = projectile;
    let mut hit = None;
    for tick in 0..ticks(4.0) {
        state = step(state, &[drive(), idle()], DT);
        if let Some(event) = spun_out(state.events()) {
            hit = Some((tick, event));
            break;
        }
        let [projectile] = state.projectiles()[..] else {
            panic!("still in flight: {:?}", state.projectiles());
        };
        let flown = ground_distance(projectile.position, last.position);
        assert!(
            (flown - speed * DT).abs() < 1e-3,
            "flew {flown} m in a tick, not {}",
            speed * DT
        );
        // Lap-relative from the start line, which the grid stands behind.
        let along = |position| track.lap_relative_progress(0, track.progress_at(position));
        assert!(
            along(projectile.position) > along(last.position),
            "flying up the spline"
        );
        if tick == ticks(0.5) {
            let on_line = track.point_at(track.progress_at(projectile.position));
            assert!(
                ground_distance(projectile.position, on_line) < 0.3,
                "merged onto the centreline: {} m off",
                ground_distance(projectile.position, on_line)
            );
        }
        last = projectile;
    }
    let (tick, event) = hit.expect("the projectile caught kart 0");
    assert_eq!(event, RaceEvent::SpunOut { kart: 0, by: 1 });
    assert!(tick > ticks(0.5), "it took a while: {tick} ticks");
    assert!(state.projectiles().is_empty(), "used up");
    let [target, owner] = state.kart_readouts()[..] else {
        unreachable!()
    };
    assert_eq!(target.speed, 0.0, "stopped");
    assert!((target.spin_out_s.expect("spun out") - SPIN_OUT_S).abs() < 1e-5);
    assert_eq!(owner.spin_out_s, None, "the owner is untouched");
}

#[test]
fn the_target_is_the_kart_next_ahead_by_race_position_not_the_leader() {
    // Kart 0 drives away; kart 1 drives briefly and coasts, second; kart 2
    // on the row behind holds the projectile, third.
    let mut state = holding(3, &[(2, Item::Homing)]);
    for tick in 0..ticks(1.5) {
        let second = if tick < ticks(0.75) { drive() } else { idle() };
        state = step(state, &[drive(), second, idle()], DT);
    }
    let positions: Vec<u32> = state.standings().iter().map(|s| s.position).collect();
    assert_eq!(positions, vec![1, 2, 3]);

    let mut state = step(state, &[drive(), idle(), tap()], DT);
    assert_eq!(state.projectiles()[0].target, 1);
    for _ in 0..ticks(4.0) {
        state = step(state, &[drive(), idle(), idle()], DT);
        if let Some(event) = spun_out(state.events()) {
            assert_eq!(event, RaceEvent::SpunOut { kart: 1, by: 2 });
            assert_eq!(
                state.kart_readouts()[0].spin_out_s,
                None,
                "the leader drives on"
            );
            return;
        }
    }
    panic!("the projectile never reached kart 1");
}

#[test]
fn a_projectile_fired_with_no_kart_ahead_is_wasted() {
    // The decision `HOMING_WITHOUT_TARGET_IS_WASTED` records.
    let state = step(holding(1, &[(0, Item::Homing)]), &[tap()], DT);
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: 0,
            item: Item::Homing
        }],
        "the fire is still reported"
    );
    assert_eq!(
        state.kart_readouts()[0].held_item,
        None,
        "the slot is empty"
    );
    assert!(state.projectiles().is_empty(), "nothing flies");

    // The same for a leader with karts behind it.
    let mut state = holding(2, &[(0, Item::Homing)]);
    for _ in 0..ticks(1.0) {
        state = step(state, &[drive(), idle()], DT);
    }
    assert_eq!(state.standings()[0].position, 1);
    let state = step(state, &[tap(), idle()], DT);
    assert!(state.projectiles().is_empty());
}

#[test]
fn a_hazard_the_target_trails_destroys_the_projectile_and_is_used_up() {
    let drive_holding = KartInput {
        throttle: 1.0,
        hold_item: true,
        ..KartInput::default()
    };
    // Kart 0 drives off trailing its hazard; kart 1 fires after it a
    // second later.
    let mut state = holding(2, &[(0, Item::Hazard), (1, Item::Homing)]);
    for _ in 0..ticks(1.0) {
        state = step(state, &[drive_holding, idle()], DT);
    }
    assert!(state.hazards()[0].held);
    let mut state = step(state, &[drive_holding, tap()], DT);
    assert_eq!(state.projectiles().len(), 1);
    let speed_at_fire = state.kart_readouts()[0].speed;
    let mut blocked_at = None;
    for tick in 0..ticks(4.0) {
        state = step(state, &[drive_holding, idle()], DT);
        assert_eq!(spun_out(state.events()), None, "nobody is spun out");
        if state.projectiles().is_empty() {
            blocked_at = Some(tick);
            break;
        }
        assert_eq!(
            state.hazards().len(),
            1,
            "the hazard trails until the projectile reaches it"
        );
    }
    assert!(
        blocked_at.is_some(),
        "the projectile never reached the hazard"
    );
    assert!(state.hazards().is_empty(), "the hazard is used up with it");
    assert!(
        state.kart_readouts()[0].speed > speed_at_fire,
        "kart 0 drives on"
    );

    // A hazard dropped on the track shields nobody: the projectile flies
    // over it to the kart.
    let mut state = step(
        holding(2, &[(0, Item::Hazard), (1, Item::Homing)]),
        &[tap(), idle()],
        DT,
    );
    for _ in 0..ticks(1.0) {
        state = step(state, &[drive(), idle()], DT);
    }
    assert!(!state.hazards()[0].held);
    let mut state = step(state, &[drive(), tap()], DT);
    for _ in 0..ticks(4.0) {
        state = step(state, &[drive(), idle()], DT);
        if spun_out(state.events()).is_some() {
            assert_eq!(
                state.hazards().len(),
                1,
                "the dropped hazard lies where it was"
            );
            return;
        }
    }
    panic!("the projectile never reached kart 0");
}

#[test]
fn a_projectile_is_spent_on_a_target_already_spun_out() {
    // Karts 1 and 2 hold projectiles. Kart 0 drives off and kart 1 fires
    // after it; kart 2 drives up past the idle kart 1 to second place and
    // fires at kart 0 while it is still spinning from the first hit. The
    // second projectile reaches a kart already spun out and is spent:
    // neither a second hit nor a longer spin-out.
    let mut state = holding(3, &[(1, Item::Homing), (2, Item::Homing)]);
    for _ in 0..ticks(1.0) {
        state = step(state, &[drive(), idle(), idle()], DT);
    }
    let mut state = step(state, &[drive(), tap(), drive()], DT);
    assert_eq!(state.projectiles()[0].target, 0);
    let mut first_hit = false;
    for _ in 0..ticks(3.0) {
        state = step(state, &[drive(), idle(), drive()], DT);
        first_hit |= spun_out(state.events()) == Some(RaceEvent::SpunOut { kart: 0, by: 1 });
        if first_hit && state.standings()[2].position == 2 {
            break;
        }
    }
    assert!(
        first_hit && state.standings()[2].position == 2,
        "kart 2 is second behind the spinning kart 0"
    );
    assert!(state.kart_readouts()[0].spin_out_s.is_some());

    let mut state = step(state, &[idle(), idle(), tap()], DT);
    assert_eq!(state.projectiles()[0].target, 0);
    for _ in 0..ticks(2.0) {
        state = step(state, &[idle(), idle(), idle()], DT);
        assert_eq!(spun_out(state.events()), None, "no second hit");
        if state.projectiles().is_empty() {
            assert!(
                state.kart_readouts()[0].spin_out_s.is_some(),
                "spent on a kart still spinning"
            );
            return;
        }
    }
    panic!("the second projectile never reached kart 0");
}

#[test]
fn a_bot_fires_a_projectile_within_the_tuned_delay_when_a_kart_is_ahead_and_holds_it_while_leading()
{
    // The bot on the second slot rolls a projectile at Go with the pole
    // bot ahead of it: it fires within the delay and the pole bot is hit.
    let mut state = holding_with_bots(0, 2, &[(1, Item::Homing)]);
    let mut fired_at = None;
    for tick in 0..ticks(BOT_ITEM_USE_DELAY_S + 4.0) {
        state = step(state, &[], DT);
        if state.events().contains(&RaceEvent::ItemFired {
            kart: 1,
            item: Item::Homing,
        }) {
            assert!(
                tick < ticks(BOT_ITEM_USE_DELAY_S),
                "fired {tick} ticks after the pickup"
            );
            fired_at = Some(tick);
        }
        if spun_out(state.events()) == Some(RaceEvent::SpunOut { kart: 0, by: 1 }) {
            assert!(fired_at.is_some());
            break;
        }
    }
    assert!(fired_at.is_some(), "the bot never fired");

    // The pole bot rolls a projectile with nobody ahead: it keeps it.
    let mut state = holding_with_bots(0, 2, &[(0, Item::Homing)]);
    for _ in 0..ticks(2.0) {
        state = step(state, &[], DT);
        assert_eq!(state.standings()[0].position, 1, "the pole bot leads");
        assert!(
            !state
                .events()
                .iter()
                .any(|event| matches!(event, RaceEvent::ItemFired { kart: 0, .. })),
            "fired with nobody ahead: {:?}",
            state.events()
        );
    }
    assert_eq!(state.kart_readouts()[0].held_item, Some(Item::Homing));
}
