//! Rain at the sim's public seam (ADR 0103): the Weather pick, the lap the
//! seed picks, and a wetness every kart shares from the first tick any kart
//! starts that lap. What wetness does to one kart's grip is in `handling`.

mod common;

use common::{oval, started, ticks, DT, SEED};

use sim::tuning::{FRESH_PICKS, PICK_CHOICES, RAIN_WETTING_S};
use sim::{bot, rain_lap, step, BotLevel, Picks, Prediction, State, Tyres, Weather};

fn picks(weather: Weather) -> Picks {
    Picks {
        level: BotLevel::Ace,
        tyres: Tyres::On,
        weather,
        ..Picks::default()
    }
}

/// A seed whose three-lap rain comes on lap 2, so a test reaches it soonest.
fn seed_of_lap_two() -> u64 {
    (SEED..)
        .find(|&seed| rain_lap(seed, 3) == 2)
        .expect("some seed picks lap 2")
}

/// A race of two Ace bots on the oval, stepped until the rain has begun.
fn raining() -> State {
    let mut state = started(State::with_picks(
        seed_of_lap_two(),
        0,
        2,
        oval(),
        picks(Weather::Rain),
    ));
    for _ in 0..ticks(60.0) {
        if state.rain_began_tick().is_some() {
            return state;
        }
        state = step(state, &[], DT);
    }
    panic!("the rain never began");
}

#[test]
fn weather_starts_on_dry_and_a_race_with_no_pick_is_dry() {
    assert_eq!(PICK_CHOICES.weather, [Weather::Dry, Weather::Rain]);
    assert_eq!(FRESH_PICKS.weather, Weather::Dry);
    assert_eq!(Picks::default().weather, Weather::Dry);
    let kept: Picks = serde_json::from_str(r#"{"level":"pro","tyres":"on","laps":5}"#).unwrap();
    assert_eq!(kept.weather, Weather::Dry);
}

#[test]
fn the_seed_picks_lap_2_to_the_lap_before_the_last_or_2_or_3_of_three() {
    let laps_of = |laps: u32| {
        (0..400)
            .map(|seed| rain_lap(seed, laps))
            .collect::<std::collections::BTreeSet<u32>>()
            .into_iter()
            .collect::<Vec<u32>>()
    };
    assert_eq!(laps_of(3), [2, 3]);
    assert_eq!(laps_of(5), [2, 3, 4]);
    assert_eq!(rain_lap(SEED, 5), rain_lap(SEED, 5), "the seed decides it");
}

#[test]
fn rain_begins_on_the_first_tick_any_kart_starts_the_seeds_lap_and_draws_nothing_from_the_random_stream(
) {
    for lap in [2, 3] {
        let seed = (SEED..).find(|&seed| rain_lap(seed, 3) == lap).unwrap();
        let race = |weather: Weather| State::with_picks(seed, 0, 2, oval(), picks(weather));
        let (mut rain, mut dry) = (race(Weather::Rain), race(Weather::Dry));
        assert_eq!(rain.hash(), dry.hash(), "the pick alone changes nothing");
        // Counted from the top of the countdown, as the server's snapshots count.
        let mut tick = 0;
        loop {
            rain = step(rain, &[], DT);
            dry = step(dry, &[], DT);
            tick += 1;
            if rain.standings().iter().any(|standing| standing.lap == lap) {
                assert_eq!(rain.rain_began_tick(), Some(tick), "lap {lap}");
                assert_eq!(rain.wetness(), 0.0, "it has only begun");
                break;
            }
            assert_eq!(rain.rain_began_tick(), None, "lap {lap}");
            assert_eq!(rain.hash(), dry.hash(), "nothing drawn before it begins");
        }
        assert_eq!(dry.rain_began_tick(), None, "a dry race never rains");
        assert_ne!(rain.hash(), dry.hash(), "the rain enters the hash");
    }
}

#[test]
fn wetness_climbs_from_0_to_1_over_its_time_and_holds_to_the_flag() {
    let mut state = raining();
    let step_for = |mut state: State, seconds: f32| {
        for _ in 0..ticks(seconds) {
            state = step(state, &[], DT);
        }
        state
    };
    state = step_for(state, RAIN_WETTING_S / 2.0);
    assert!((state.wetness() - 0.5).abs() < 1e-3, "{}", state.wetness());
    state = step_for(state, RAIN_WETTING_S / 2.0);
    assert_eq!(state.wetness(), 1.0);
    state = step_for(state, 1.0);
    assert_eq!(state.wetness(), 1.0, "it never dries");
}

#[test]
fn a_prediction_never_judges_the_rain_itself_and_reads_the_snapshots_word_on_the_servers_ticks() {
    let seed = seed_of_lap_two();
    let mut prediction = Prediction::new(seed, 0, oval(), picks(Weather::Rain));
    let mut memory = bot::Memory::default();
    let mut tick = 0;
    let mut drive = |prediction: &mut Prediction| {
        let input = bot::drive(
            prediction.track(),
            BotLevel::Ace.profile(),
            &[prediction.kart_readout()],
            &[prediction.standing()],
            prediction.phase(),
            seed,
            sim::tuning::DEFAULT_LAPS,
            0,
            &mut memory,
            prediction.wetness(),
            DT,
        );
        tick += 1;
        prediction.tick(tick, input, &[]);
        tick
    };
    // Its own kart reaching the seed's lap says nothing: another may have
    // got there sooner.
    let mut last = 0;
    while prediction.standing().lap < 2 {
        last = drive(&mut prediction);
        assert!(last < ticks(60.0) as u32, "never reached lap 2");
    }
    assert_eq!(prediction.wetness(), 0.0);

    let half_wet = last - ticks(RAIN_WETTING_S / 2.0) as u32;
    prediction.adopt_rain_began_tick(half_wet);
    assert!((prediction.wetness() - 0.5).abs() < 1e-3);

    // A rewind replays the inputs since the snapshot and ends on the same tick.
    let readout = prediction.kart_readout();
    prediction.reconcile(last - 5, last - 5, &readout);
    assert!(
        (prediction.wetness() - 0.5).abs() < 1e-3,
        "{}",
        prediction.wetness()
    );

    for _ in 0..ticks(RAIN_WETTING_S) {
        drive(&mut prediction);
    }
    assert_eq!(prediction.wetness(), 1.0);
}

#[test]
fn rain_slows_a_field_of_bots() {
    // The bots share the steering rate and the ramps, so a wet field takes
    // longer over the same laps than a dry one from the same seed.
    let seed = seed_of_lap_two();
    let winning_time = |weather: Weather| {
        let mut state = started(State::with_picks(seed, 0, 2, oval(), picks(weather)));
        for _ in 0..ticks(120.0) {
            state = step(state, &[], DT);
            if let Some(time) = state
                .standings()
                .iter()
                .find_map(|standing| standing.finish_time_s)
            {
                return time;
            }
        }
        panic!("nobody finished");
    };
    assert!(winning_time(Weather::Dry) < winning_time(Weather::Rain));
}
