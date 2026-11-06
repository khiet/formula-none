//! Bots and the pit lane (ADR 0081), through `step`: a bot calls itself in by
//! its level's rule, takes the lane's road to its pit box and rejoins; the
//! whole field gets round every course on Tyres On. The call's rule itself is
//! read off [`sim::bot::calls_for_the_pits`] and [`sim::bot::saves_its_tyres`]
//! with made-up numbers, since a race does not reach every case on demand.

mod common;

use common::{
    along_lane, bell_deer, goldstone, monarch, oval, quiet_bell_deer, started, ticks, track,
    with_slot_at, with_slot_on_lane, without_item_boxes, BELL_DEER, DT, GOLDSTONE, MONARCH, SEED,
};
use sim::bot::{calls_for_the_pits, saves_its_tyres, PitReading};
use sim::tuning::{
    BotPitCall, BOT_LEVELS, BOT_TYRE_LOSS_S_PER_LAP, COUNTDOWN_S, PIT_LIMIT_M_PER_S,
    TYRE_LIFE_CLIFF_PERCENT, TYRE_LIFE_NEW_PERCENT,
};
use sim::Picks;
use sim::{step, BotLevel, Item, ItemSlot, RaceEvent, State, Surface, Track, Tyres};

/// A course by name, built afresh for each race.
type Course = (&'static str, fn() -> Track);

const LANES: [Course; 3] = [
    ("Bell Deer", bell_deer),
    ("Goldstone", goldstone),
    ("Monarch", monarch),
];

const COURSES: [Course; 4] = [
    ("oval", oval),
    ("Bell Deer", bell_deer),
    ("Goldstone", goldstone),
    ("Monarch", monarch),
];

fn reading(life: f32, wear_per_lap: Option<f32>, laps_to_go: u32) -> PitReading {
    PitReading {
        life,
        wear_per_lap,
        laps_to_go,
    }
}

fn level_call(level: BotLevel) -> BotPitCall {
    BOT_LEVELS[level as usize].pit_call
}

#[test]
fn every_level_comes_in_by_the_cliff_with_a_lap_or_more_to_go() {
    for level in BotLevel::ALL {
        let call = level_call(level);
        assert!(
            calls_for_the_pits(call, reading(30.0, None, 1)),
            "{level:?}"
        );
        // 20 a lap from 45 ends the next lap on 25, past the cliff.
        assert!(
            calls_for_the_pits(call, reading(45.0, Some(20.0), 3)),
            "{level:?}"
        );
        assert!(
            !calls_for_the_pits(call, reading(60.0, Some(20.0), 1)),
            "{level:?}"
        );
        assert!(
            !calls_for_the_pits(call, reading(5.0, None, 0)),
            "{level:?}: the flag is at the line ahead"
        );
    }
}

#[test]
fn an_ace_bot_stays_out_when_its_tyres_reach_the_flag_at_the_races_count() {
    let call = level_call(BotLevel::Ace);
    // 25 a lap from 55: over one lap more it ends on the cliff, and stays out;
    // over two it ends on 5, and a stop now pays, with no later lap left to
    // wait for.
    assert!(!calls_for_the_pits(call, reading(55.0, Some(25.0), 1)));
    assert!(calls_for_the_pits(call, reading(30.0, Some(25.0), 2)));
}

#[test]
fn an_ace_bot_holds_for_the_stop_that_saves_the_most_time() {
    let call = level_call(BotLevel::Ace);
    // Four laps to go at 25 a lap from 75: new tyres now would run out by the
    // flag; a lap on they reach it with life left.
    assert!(!calls_for_the_pits(call, reading(75.0, Some(25.0), 4)));
    // Three to go at 20 a lap from 55: the next lane is above the cliff, but
    // staying out ends on 5, and no later lap's stop costs less than now.
    assert!(calls_for_the_pits(call, reading(55.0, Some(20.0), 3)));
}

#[test]
fn an_ace_bot_projects_nothing_from_no_stint() {
    let call = level_call(BotLevel::Ace);
    assert!(!calls_for_the_pits(call, reading(40.0, None, 4)));
}

#[test]
fn an_ace_bot_saves_its_tyres_only_when_saving_is_what_reaches_the_flag() {
    let call = level_call(BotLevel::Ace);
    // 20 a lap from 60 over two: pushing ends on 20, saving a third ends on 33.
    assert!(saves_its_tyres(call, reading(60.0, Some(20.0), 2)));
    // Pushing already ends on the cliff.
    assert!(!saves_its_tyres(call, reading(70.0, Some(20.0), 2)));
    // Even saving ends on 23.
    assert!(!saves_its_tyres(call, reading(50.0, Some(20.0), 2)));
    assert!(!saves_its_tyres(call, reading(60.0, None, 2)));
    assert!(
        !saves_its_tyres(call, reading(35.0, Some(20.0), 0)),
        "the flag is at the line ahead"
    );
}

#[test]
fn an_ace_bot_that_saving_gets_to_the_flag_stays_out_where_pushing_would_stop() {
    let call = level_call(BotLevel::Ace);
    // Pushing ends on 20 and a stop pays; saving ends on about 33.
    let close = reading(60.0, Some(20.0), 2);
    assert!(saves_its_tyres(call, close));
    assert!(!calls_for_the_pits(call, close));
    // Pushing would wear past the cliff by the next lane, at 25; saving keeps
    // it on about 32 at the flag, so the cliff does not call it in either.
    let last_lap = reading(45.0, Some(20.0), 1);
    assert!(saves_its_tyres(call, last_lap));
    assert!(!calls_for_the_pits(call, last_lap));
}

#[test]
fn only_an_ace_bot_saves_its_tyres() {
    for level in [BotLevel::Basic, BotLevel::Pro] {
        assert!(!saves_its_tyres(
            level_call(level),
            reading(60.0, Some(20.0), 2)
        ));
    }
}

/// What a race did, per bot: the laps it stopped on, whether it finished
/// and how many ticks it spent on grass.
struct Raced {
    stop_laps: Vec<Vec<u32>>,
    finished: Vec<bool>,
    grass_ticks: Vec<u32>,
    shortest_lap: u32,
}

fn race(seed: u64, course: Track, level: BotLevel, tyres: Tyres, laps: u32) -> Raced {
    let mut state = started(State::with_picks(
        seed,
        0,
        8,
        course,
        Picks { level, tyres, laps },
    ));
    let mut raced = Raced {
        stop_laps: vec![Vec::new(); 8],
        finished: vec![false; 8],
        grass_ticks: vec![0; 8],
        shortest_lap: 0,
    };
    let mut stops = [0; 8];
    while matches!(state.phase(), sim::Phase::Racing { .. }) {
        state = step(state, &[], DT);
        let readouts = state.kart_readouts();
        let standings = state.standings();
        for slot in 0..8 {
            if readouts[slot].stops != stops[slot] {
                stops[slot] = readouts[slot].stops;
                raced.stop_laps[slot].push(standings[slot].lap);
            }
            raced.grass_ticks[slot] += u32::from(state.kart_surface(slot) == Surface::Grass);
            raced.finished[slot] = standings[slot].finish_time_s.is_some();
        }
        raced.shortest_lap = standings.iter().map(|s| s.lap).min().unwrap_or(0);
    }
    raced
}

/// A lone bot's stops and finish over five laps.
fn lone_five_laps(course: Track, level: BotLevel) -> (u32, Option<f32>) {
    lone_five_laps_from(SEED, course, level)
}

fn lone_five_laps_from(seed: u64, course: Track, level: BotLevel) -> (u32, Option<f32>) {
    let mut state = started(State::with_picks(
        seed,
        0,
        1,
        course,
        Picks {
            level,
            tyres: Tyres::On,
            laps: 5,
        },
    ));
    for _ in 0..ticks(5.0 * 120.0) {
        state = step(state, &[], DT);
        if state.standings()[0].finish_time_s.is_some() {
            break;
        }
    }
    (
        state.kart_readouts()[0].stops,
        state.standings()[0].finish_time_s,
    )
}

#[test]
fn a_lone_basic_bot_takes_monarchs_lane_and_finishes() {
    let (stops, finish) = lone_five_laps(monarch(), BotLevel::Basic);
    assert!(stops >= 1 && finish.is_some(), "{stops} stops, {finish:?}");
}

#[test]
fn a_lone_pro_bot_takes_goldstones_lane_and_finishes() {
    let (stops, finish) = lone_five_laps(goldstone(), BotLevel::Pro);
    assert!(stops >= 1 && finish.is_some(), "{stops} stops, {finish:?}");
}

#[test]
fn a_lone_ace_bot_takes_bell_deers_lane_and_finishes() {
    let (stops, finish) = lone_five_laps(bell_deer(), BotLevel::Ace);
    assert!(stops >= 1 && finish.is_some(), "{stops} stops, {finish:?}");
}

#[test]
fn a_bot_on_tyres_off_never_takes_the_lane() {
    let mut state = started(State::with_picks(
        SEED,
        0,
        1,
        goldstone(),
        Picks {
            level: BotLevel::Ace,
            laps: 3,
            ..Picks::default()
        },
    ));
    for _ in 0..ticks(3.0 * 120.0) {
        state = step(state, &[], DT);
        assert!(!state.kart_readouts()[0].in_pit_lane);
        if state.standings()[0].finish_time_s.is_some() {
            break;
        }
    }
    assert!(state.standings()[0].finish_time_s.is_some());
    assert_eq!(state.kart_readouts()[0].stops, 0);
}

#[test]
fn a_kart_handed_to_a_bot_between_the_lines_drives_on_down_the_lane() {
    // Dropped behind the pit wall with no call made: the wall leaves it one
    // road. The second human, idle on the grid, holds the race open.
    let sidecar = with_slot_on_lane(&quiet_bell_deer(), 0, 40.0);
    let mut state = started(State::with_bots(
        SEED,
        2,
        0,
        track(&sidecar),
        BotLevel::Basic,
    ));
    state.hand_to_bot(0);
    for _ in 0..ticks(15.0) {
        state = step(state, &[sim::KartInput::default(); 2], DT);
    }
    let kart = state.kart_readouts()[0];
    assert!(!kart.in_pit_lane, "out past the exit line: {kart:?}");
    assert!(
        kart.speed > PIT_LIMIT_M_PER_S,
        "and away at racing speed: {}",
        kart.speed
    );
}

/// A lone bot put on Bell Deer's centreline at the spline progress `progress`
/// picks, past its countdown.
fn lone_bot_at(progress: impl FnOnce(&Track) -> f32, level: BotLevel) -> State {
    let course = bell_deer();
    let at = progress(&course);
    let here = course.point_at(at);
    let ahead = course.point_at(at + 1.0);
    let yaw = f32::atan2(here[0] - ahead[0], here[2] - ahead[2]);
    let sidecar = with_slot_at(&quiet_bell_deer(), 0, here, yaw);
    started(State::with_picks(
        SEED,
        0,
        1,
        track(&sidecar),
        Picks {
            level,
            tyres: Tyres::On,
            ..Picks::default()
        },
    ))
}

/// What the bot asks for on one look at `state`, on tyres at `life` and holding `item`.
fn asks(state: &State, level: BotLevel, life: f32, item: ItemSlot) -> sim::KartInput {
    let mut kart = state.kart_readouts()[0];
    kart.tyre_life = life;
    kart.item_slot = item;
    kart.speed = 15.0;
    sim::bot::drive(
        state.track(),
        level.profile(),
        &[kart],
        &state.standings(),
        state.phase(),
        SEED,
        3,
        0,
        &mut sim::bot::Memory::default(),
        DT,
    )
}

#[test]
fn called_in_a_bot_keeps_its_boost_for_after_the_lane() {
    let state = lone_bot_at(
        |course| course.pit_lane_fork().expect("a lane") - 20.0,
        BotLevel::Basic,
    );
    let boost = ItemSlot::Held(Item::Boost);
    assert!(
        asks(&state, BotLevel::Basic, 100.0, boost).use_item,
        "on new tyres it fires"
    );
    assert!(
        !asks(&state, BotLevel::Basic, 5.0, boost).use_item,
        "called in, it does not"
    );
}

/// A point past the grid just short of a tight corner, where a bot hops for it.
fn hairpin(course: &Track) -> f32 {
    (0..course.spline_length() as usize)
        .map(|at| at as f32)
        .find(|&at| course.curvature_at(at + 12.0).abs() > 0.05 && at > 150.0)
        .expect("a tight corner")
}

#[test]
fn a_pro_bot_drops_its_drifts_on_worn_tyres_and_a_basic_bot_does_not() {
    let pro = lone_bot_at(hairpin, BotLevel::Pro);
    assert!(asks(&pro, BotLevel::Pro, 80.0, ItemSlot::Empty).hop);
    assert!(!asks(&pro, BotLevel::Pro, 20.0, ItemSlot::Empty).hop);
    let basic = lone_bot_at(hairpin, BotLevel::Basic);
    assert!(asks(&basic, BotLevel::Basic, 20.0, ItemSlot::Empty).hop);
}

#[test]
fn an_ace_bot_saving_its_tyres_drops_its_drifts_until_it_reads_otherwise() {
    let mut memory = sim::bot::Memory::default();
    let mut look = |state: &State, life: f32| {
        let mut kart = state.kart_readouts()[0];
        kart.tyre_life = life;
        kart.speed = 15.0;
        sim::bot::drive(
            state.track(),
            BotLevel::Ace.profile(),
            &[kart],
            &state.standings(),
            state.phase(),
            SEED,
            3,
            0,
            &mut memory,
            DT,
        )
    };
    let approach = lone_bot_at(
        |course| course.pit_lane_fork().expect("a lane") - 20.0,
        BotLevel::Ace,
    );
    let corner = lone_bot_at(hairpin, BotLevel::Ace);
    assert!(look(&corner, 74.0).hop, "nothing read yet");
    look(&approach, 74.0);
    assert!(!look(&corner, 74.0).hop, "saving reaches the flag");
    look(&approach, 95.0);
    assert!(look(&corner, 95.0).hop, "pushing reaches the flag");
}

/// On Bell Deer three laps is where a lone Ace bot's clean 25% a lap ends past
/// the cliff pushing and reaches the flag above it saving; four wants a stop.
#[test]
fn a_lone_ace_bot_saves_its_way_to_the_flag_with_no_stop() {
    let mut state = started(State::with_picks(
        SEED,
        0,
        1,
        bell_deer(),
        Picks {
            level: BotLevel::Ace,
            tyres: Tyres::On,
            laps: 3,
        },
    ));
    for _ in 0..ticks(3.0 * 120.0) {
        state = step(state, &[], DT);
        if state.standings()[0].finish_time_s.is_some() {
            break;
        }
    }
    let kart = state.kart_readouts()[0];
    assert!(state.standings()[0].finish_time_s.is_some());
    assert_eq!(kart.stops, 0);
    assert!(
        kart.tyre_life >= TYRE_LIFE_CLIFF_PERCENT,
        "it reached the flag on {}",
        kart.tyre_life
    );
}

/// Three laps is the Sprint ADR 0083 kept a managing no-stop viable at, and
/// ADR 0087's steeper factors keep it so: a lone Ace bot gets round every
/// course with a lane on the tyres it started on.
#[test]
fn a_lone_ace_bot_runs_three_laps_of_every_lane_course_without_a_stop() {
    for (name, course) in LANES {
        let mut state = started(State::with_picks(
            SEED,
            0,
            1,
            course(),
            Picks {
                level: BotLevel::Ace,
                tyres: Tyres::On,
                laps: 3,
            },
        ));
        for _ in 0..ticks(3.0 * 120.0) {
            state = step(state, &[], DT);
            if state.standings()[0].finish_time_s.is_some() {
                break;
            }
        }
        let kart = state.kart_readouts()[0];
        assert!(state.standings()[0].finish_time_s.is_some(), "{name}");
        assert_eq!(kart.stops, 0, "{name}");
        assert!(
            kart.tyre_life > 0.0,
            "{name}: it reached the flag on {}",
            kart.tyre_life
        );
    }
}

/// Most seeds' read of the first lap lands a lone bot within reach here, not all,
/// so any of a handful will do.
#[test]
fn a_lone_ace_bot_can_save_its_way_through_five_laps_of_goldstone() {
    assert!((0..10).any(|seed| {
        let (stops, finish) = lone_five_laps_from(seed, goldstone(), BotLevel::Ace);
        stops == 0 && finish.is_some()
    }));
}

#[test]
fn a_bot_driven_through_the_countdown_does_not_back_off_the_line() {
    let state = State::with_picks(SEED, 1, 0, oval(), Picks::default());
    let mut memory = sim::bot::Memory::default();
    for _ in 0..ticks(COUNTDOWN_S) {
        let input = sim::bot::drive(
            state.track(),
            BotLevel::Basic.profile(),
            &state.kart_readouts(),
            &state.standings(),
            state.phase(),
            SEED,
            3,
            0,
            &mut memory,
            DT,
        );
        assert_eq!(input.brake, 0.0);
    }
}

/// A number from the environment variable `name`, or `default` when it is unset.
fn from_env(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a whole number, not {value:?}"))
    })
}

/// Races the sweeps run at once across every test, `SWEEP_THREADS` or 4, so a
/// sweep leaves the rest of the machine usable.
fn sweep_threads() -> usize {
    from_env("SWEEP_THREADS", 4).max(1) as usize
}

/// The races running now, held under `sweep_threads` by `on_a_sweep_thread`.
static RUNNING: (std::sync::Mutex<usize>, std::sync::Condvar) =
    (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// `job` on a thread of its own, which waits for a free sweep slot first.
fn on_a_sweep_thread<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> std::thread::JoinHandle<T> {
    std::thread::spawn(move || {
        let (running, freed) = &RUNNING;
        let mut count = freed
            .wait_while(running.lock().unwrap(), |count| *count >= sweep_threads())
            .unwrap();
        *count += 1;
        drop(count);
        let done = job();
        *running.lock().unwrap() -= 1;
        freed.notify_one();
        done
    })
}

/// Every race of the sweep across `seeds`, a few at a time.
fn sweep(
    seeds: std::ops::Range<u64>,
    course: fn() -> Track,
    level: BotLevel,
    laps: u32,
) -> Vec<(u64, Raced)> {
    let races: Vec<_> = seeds
        .map(|seed| on_a_sweep_thread(move || (seed, race(seed, course(), level, Tyres::On, laps))))
        .collect();
    races
        .into_iter()
        .map(|race| race.join().expect("the race ran"))
        .collect()
}

/// Bots at one level do not all pit on one lap. Too slow for a debug build; run
/// it with the sweep below.
#[test]
#[ignore]
fn over_five_laps_a_levels_bots_do_not_all_pit_on_one_lap() {
    for (name, course) in LANES {
        for level in BotLevel::ALL {
            let races = sweep(0..40, course, level, 5);
            let bunched = races.iter().filter(|(_, raced)| {
                let firsts: Vec<u32> = raced
                    .stop_laps
                    .iter()
                    .filter_map(|laps| laps.first().copied())
                    .collect();
                firsts.len() == 8 && firsts.iter().all(|&lap| lap == firsts[0])
            });
            let bunched = bunched.count();
            // Bell Deer's fork comes early in the lap, and a Basic pack wears
            // past the cliff by the second lap's fork almost to a kart, so its
            // first stops bunch there (ADR 0095).
            let most = if name == "Bell Deer" && level == BotLevel::Basic {
                races.len() / 2
            } else {
                races.len() / 5
            };
            assert!(
                bunched <= most,
                "{name} {level:?}: all eight on one lap in {bunched} of {} races",
                races.len()
            );
        }
    }
}

/// The most karts of 1600, 200 races of eight, the end cap may catch over three
/// laps and over five, and that may touch grass in a sweep. ADR 0085 has why
/// these are not zero: the cap stays as ADR 0083 left it, and a kart launched
/// over a wall lands where it lands, as it did before bots pitted.
const CAPPED_OVER_THREE_LAPS: usize = 4;
const CAPPED_OVER_FIVE_LAPS: usize = 24;
const ON_GRASS: usize = 40;
/// Races of eight, of 200, in which a kart may end short of the final lap.
const SHORT_OF_THE_FINAL_LAP: usize = 2;
/// The races of eight the budgets above are set over.
const BUDGET_RACES: u64 = 200;

/// A budget set over `BUDGET_RACES` scaled to a sweep of `races`, rounded up.
fn scaled(budget: usize, races: u64) -> usize {
    (budget as u64 * races).div_ceil(BUDGET_RACES) as usize
}

/// Every bot gets round every course at every level on Tyres On, within the
/// budgets above scaled to the sweep. Too slow for a debug build; run with
/// `cargo test --release -p sim --test bot_pits -- --ignored --nocapture field_gets_round`.
/// It races `SWEEP_SEEDS` seeds, 40 by default, enough to show a rate that
/// doubles; compare a change against main on the same seeds. `SWEEP_SEEDS=200`
/// is for setting the budgets again, and takes about 15 minutes on every core
/// with `SWEEP_THREADS` raised to match.
#[test]
#[ignore]
fn in_a_sweep_of_eight_bots_on_tyres_on_the_field_gets_round() {
    let seeds = from_env("SWEEP_SEEDS", 40);
    let mut refused = Vec::new();
    for (name, course) in COURSES {
        for level in BotLevel::ALL {
            for laps in [3, 5] {
                let races = sweep(0..seeds, course, level, laps);
                let count = |of: &dyn Fn(&Raced) -> usize| -> usize {
                    races.iter().map(|(_, raced)| of(raced)).sum()
                };
                let capped = count(&|raced| raced.finished.iter().filter(|&&done| !done).count());
                let on_grass =
                    count(&|raced| raced.grass_ticks.iter().filter(|&&ticks| ticks > 0).count());
                let short = count(&|raced| usize::from(raced.shortest_lap < laps));
                println!(
                    "{name} {level:?} {laps} laps: {capped} capped, {short} short, {on_grass} on grass, of {} karts",
                    races.len() * 8
                );
                let most_capped = scaled(
                    if laps == 3 {
                        CAPPED_OVER_THREE_LAPS
                    } else {
                        CAPPED_OVER_FIVE_LAPS
                    },
                    seeds,
                );
                if capped > most_capped
                    || on_grass > scaled(ON_GRASS, seeds)
                    || short > scaled(SHORT_OF_THE_FINAL_LAP, seeds)
                {
                    refused.push(format!(
                        "{name} {level:?} {laps} laps: {capped} capped, {short} short, {on_grass} on grass"
                    ));
                }
            }
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
}

/// The mean of a lone Ace bot's second and third laps on `course` with its tyres
/// held at `life` every tick, over `seeds`: nothing but the tyres moves the lap.
fn lap_time_held_at(course: fn() -> Track, life: f32, seeds: std::ops::Range<u64>) -> f32 {
    let count = (seeds.end - seeds.start) as f32;
    let laps: Vec<_> = seeds
        .map(|seed| {
            on_a_sweep_thread(move || {
                let mut state = started(State::with_picks(
                    seed,
                    0,
                    1,
                    course(),
                    Picks {
                        level: BotLevel::Ace,
                        tyres: Tyres::On,
                        laps: 3,
                    },
                ));
                let mut timed = Vec::new();
                while matches!(state.phase(), sim::Phase::Racing { .. }) {
                    state.adopt_tyre_life(0, life);
                    state = step(state, &[], DT);
                    timed.extend(state.events().iter().filter_map(|event| match event {
                        RaceEvent::LapCompleted {
                            lap, lap_time_s, ..
                        } if *lap >= 2 => Some(*lap_time_s),
                        _ => None,
                    }));
                }
                assert_eq!(timed.len(), 2, "seed {seed} at {life}%: {timed:?}");
                timed.iter().sum::<f32>() / 2.0
            })
        })
        .collect();
    laps.into_iter()
        .map(|lap| lap.join().expect("the race ran"))
        .sum::<f32>()
        / count
}

/// A lone bot's first lap of the oval with its tyres held at `life` every tick.
fn first_lap_held_at(life: f32) -> f32 {
    let mut state = started(State::with_picks(
        SEED,
        0,
        1,
        oval(),
        Picks {
            level: BotLevel::Ace,
            tyres: Tyres::On,
            laps: 3,
        },
    ));
    for _ in 0..ticks(180.0) {
        state.adopt_tyre_life(0, life);
        state = step(state, &[], DT);
        let lapped = state.events().iter().find_map(|event| match event {
            RaceEvent::LapCompleted { lap_time_s, .. } => Some(*lap_time_s),
            _ => None,
        });
        if let Some(lap_time_s) = lapped {
            return lap_time_s;
        }
    }
    panic!("the bot never finished a lap");
}

/// The seam the lap costs are measured at: a bot's kart drives on the life
/// it is handed, and worn right out it laps slower than on new tyres.
#[test]
fn a_bot_held_on_worn_out_tyres_laps_slower_than_one_held_on_new() {
    let worn_out = first_lap_held_at(0.0);
    let new = first_lap_held_at(TYRE_LIFE_NEW_PERCENT);
    assert!(
        new < worn_out,
        "{new} s on new tyres, {worn_out} s worn out"
    );
}

/// The seconds a lap costs at each life of `BOT_TYRE_LOSS_S_PER_LAP`, measured
/// by a lone Ace bot held at that life on each course with a lane, raced without
/// the lane and its item boxes (ADR 0087). The rows are the mean over the three
/// courses, and each must sit within a second of it; the printout is the table to
/// paste when they do not. Too slow for a debug build; run it with
/// `cargo test --release -p sim --test bot_pits -- --ignored --nocapture measured`.
#[test]
#[ignore]
fn the_lap_costs_an_ace_bot_weighs_are_measured_on_the_courses_with_a_lane() {
    fn bell_deer_bare() -> Track {
        track(&without_item_boxes(&without_its_lane_sidecar(BELL_DEER)))
    }
    fn goldstone_bare() -> Track {
        track(&without_item_boxes(&without_its_lane_sidecar(GOLDSTONE)))
    }
    fn monarch_bare() -> Track {
        track(&without_item_boxes(&without_its_lane_sidecar(MONARCH)))
    }
    let bare: [Course; 3] = [
        ("Bell Deer", bell_deer_bare),
        ("Goldstone", goldstone_bare),
        ("Monarch", monarch_bare),
    ];
    let new = BOT_TYRE_LOSS_S_PER_LAP[0][0];
    let on_new: Vec<f32> = bare
        .iter()
        .map(|&(_, course)| lap_time_held_at(course, new, 0..4))
        .collect();
    let mut off = Vec::new();
    let mut rows = Vec::new();
    for [life, in_the_table] in BOT_TYRE_LOSS_S_PER_LAP {
        let costs: Vec<(&str, f32)> = bare
            .iter()
            .zip(&on_new)
            .map(|(&(name, course), on_new)| (name, lap_time_held_at(course, life, 0..4) - on_new))
            .collect();
        let mean = costs.iter().map(|(_, cost)| cost).sum::<f32>() / costs.len() as f32;
        println!("{life:>5}%: {mean:5.1} s a lap, {costs:?}");
        rows.push(format!("    [{life:.1}, {mean:.1}],"));
        if (mean - in_the_table).abs() > 1.0 {
            off.push(format!(
                "{life}%: {in_the_table} s in the table, {mean:.1} s measured"
            ));
        }
    }
    println!("{}", rows.join("\n"));
    assert!(off.is_empty(), "{}", off.join("\n"));
}

/// The sidecar's course without its lane, for a race nobody can stop in.
fn without_its_lane(sidecar: &str) -> Track {
    track(&without_its_lane_sidecar(sidecar))
}

/// `sidecar` without its lane, for `without_its_lane` and for a course to edit
/// further before it is built.
fn without_its_lane_sidecar(sidecar: &str) -> String {
    let mut parsed: serde_json::Value = serde_json::from_str(sidecar).expect("valid JSON");
    parsed
        .as_object_mut()
        .expect("an object")
        .remove("pit_lane");
    parsed.to_string()
}

/// The mean winning time over `seeds` of Ace bots on five laps.
fn ace_winning_time(course: fn() -> Track, seeds: std::ops::Range<u64>) -> f32 {
    let races: Vec<_> = seeds
        .map(|seed| {
            on_a_sweep_thread(move || {
                let mut state = started(State::with_picks(
                    seed,
                    0,
                    8,
                    course(),
                    Picks {
                        level: BotLevel::Ace,
                        tyres: Tyres::On,
                        laps: 5,
                    },
                ));
                while matches!(state.phase(), sim::Phase::Racing { .. }) {
                    state = step(state, &[], DT);
                }
                state
                    .standings()
                    .iter()
                    .filter_map(|standing| standing.finish_time_s)
                    .fold(f32::INFINITY, f32::min)
            })
        })
        .collect();
    let count = races.len() as f32;
    races
        .into_iter()
        .map(|race| race.join().expect("the race ran"))
        .sum::<f32>()
        / count
}

/// The model's finding that one stop beats none at five laps (ADR 0083), as
/// far as bots can show it; a human race is the maintainer's to judge. Too slow
/// for a debug build.
#[test]
#[ignore]
fn over_five_laps_a_field_that_can_stop_wins_sooner_than_one_that_cannot() {
    fn bell_deer_bare() -> Track {
        without_its_lane(BELL_DEER)
    }
    fn goldstone_bare() -> Track {
        without_its_lane(GOLDSTONE)
    }
    fn monarch_bare() -> Track {
        without_its_lane(MONARCH)
    }
    let bare: [fn() -> Track; 3] = [bell_deer_bare, goldstone_bare, monarch_bare];
    for ((name, with_a_lane), without_one) in LANES.into_iter().zip(bare) {
        let stopping = ace_winning_time(with_a_lane, 0..40);
        let staying_out = ace_winning_time(without_one, 0..40);
        println!("{name}: {stopping} s with a lane, {staying_out} s without");
        assert!(
            stopping < staying_out,
            "{name}: {stopping} s with a lane, {staying_out} s without"
        );
    }
}

#[test]
fn called_in_a_bot_short_of_the_lanes_first_point_reads_no_turn_there() {
    // A kart short of the path reads 0 m along it, where the turn must be read
    // from the road ahead and not from a chord of no length. Monarch's lane
    // runs nearly straight from its fork, so nothing there calls for the brake.
    let course = monarch();
    let fork = course.pit_lane_fork().expect("a lane");
    let lane = course.pit_lane().expect("a lane");
    let (start, yaw) = along_lane(&course, 0.0);
    let ahead = lane.point_at(1.0);
    let short = [
        2.0 * start[0] - ahead[0],
        start[1],
        2.0 * start[2] - ahead[2],
    ];
    let on_the_call = course.point_at(fork - 20.0);
    let towards = course.point_at(fork - 19.0);
    let call_yaw = f32::atan2(on_the_call[0] - towards[0], on_the_call[2] - towards[2]);
    let mut memory = sim::bot::Memory::default();
    let mut drive_from = |position: [f32; 3], yaw: f32| {
        let state = started(State::with_picks(
            SEED,
            0,
            1,
            track(&with_slot_at(MONARCH, 0, position, yaw)),
            Picks {
                level: BotLevel::Pro,
                tyres: Tyres::On,
                ..Picks::default()
            },
        ));
        let mut kart = state.kart_readouts()[0];
        kart.tyre_life = 5.0;
        kart.speed = PIT_LIMIT_M_PER_S;
        sim::bot::drive(
            state.track(),
            BotLevel::Pro.profile(),
            &[kart],
            &state.standings(),
            state.phase(),
            SEED,
            3,
            0,
            &mut memory,
            DT,
        )
    };
    drive_from(on_the_call, call_yaw);
    let input = drive_from(short, yaw);
    assert_eq!(input.brake, 0.0, "{input:?}");
}
