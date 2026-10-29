//! Tracks through the public seam: load a sidecar, start a race on it, ask
//! what is under a point, drive into things. The exported fixtures under
//! `assets/tracks` are the inputs; their layouts document the numbers used.

mod common;

use common::{
    brands_patch, oval, started, throttle, ticks, track, with_mesh_centred_on, with_slot_at, yaw,
    DT, OVAL, SEED,
};

use sim::tuning::{KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M, WHEEL_LANDING_SPREAD_M, WHEEL_RADIUS_M};
use sim::{step, KartInput, KartReadout, State, Surface, Track, TrackError};

/// Nearest centreline index to a point, for ordering things along the lap.
fn centreline_index(track: &Track, point: [f32; 3]) -> usize {
    let distance = |a: [f32; 3]| (0..3).map(|i| (a[i] - point[i]).powi(2)).sum::<f32>();
    (0..track.centreline().len())
        .min_by(|&a, &b| {
            distance(track.centreline()[a]).total_cmp(&distance(track.centreline()[b]))
        })
        .expect("a centreline has points")
}

#[test]
fn the_oval_fixture_has_twelve_checkpoints_on_a_438_metre_lap() {
    let track = oval();
    // Two 125 m straights joined by 30 m radius turns (layouts/oval.py).
    let expected_length = 2.0 * 125.0 + 2.0 * std::f32::consts::PI * 30.0;

    assert_eq!(track.checkpoints().len(), 12);
    assert!(
        (track.spline_length() - expected_length).abs() < 0.5,
        "lap is {} m, expected about {expected_length}",
        track.spline_length()
    );
}

#[test]
fn brands_patch_carries_the_whole_layout() {
    let track = brands_patch();

    assert_eq!(track.name(), "brands-patch");
    assert_eq!(track.checkpoints().len(), 15);
    assert_eq!(track.item_boxes().len(), 12);
    assert_eq!(track.boost_pads().len(), 2);
    assert_eq!(track.kerbs().len(), 2);
    assert_eq!(track.spawn_grid().len(), 8);
    assert!(
        (1280.0..=1330.0).contains(&track.spline_length()),
        "about 1306 m, got {}",
        track.spline_length()
    );
    for surface in [
        Surface::Asphalt,
        Surface::Sand,
        Surface::Grass,
        Surface::Wall,
        Surface::Tyres,
    ] {
        assert!(
            track.surfaces().iter().any(|mesh| mesh.surface == surface),
            "no {surface:?} mesh"
        );
    }
}

#[test]
fn item_boxes_stand_in_three_rows_of_four() {
    let track = brands_patch();
    let mut rows: Vec<usize> = track
        .item_boxes()
        .iter()
        .map(|item| centreline_index(&track, item.position))
        .collect();
    rows.sort_unstable();
    rows.dedup();
    assert_eq!(
        rows.len(),
        3,
        "boxes sit at three points along the lap: {rows:?}"
    );
}

#[test]
fn checkpoints_run_in_lap_order_from_the_start_line() {
    for track in [oval(), brands_patch()] {
        let indices: Vec<usize> = track
            .checkpoints()
            .iter()
            .map(|checkpoint| centreline_index(&track, checkpoint.position))
            .collect();
        assert_eq!(
            indices[0],
            0,
            "{}: the first checkpoint is the start line",
            track.name()
        );
        assert!(
            indices.windows(2).all(|pair| pair[0] < pair[1]),
            "{}: checkpoints out of order along the centreline: {indices:?}",
            track.name()
        );
    }
}

#[test]
fn spline_progress_runs_from_the_start_line_through_each_checkpoint_and_back_to_its_point() {
    for track in [oval(), brands_patch()] {
        let lap = track.spline_length();
        let progress = track.checkpoint_progress();
        assert_eq!(progress.len(), track.checkpoints().len());
        assert_eq!(
            progress[0],
            0.0,
            "{}: the start line is at zero",
            track.name()
        );
        assert!(
            progress.windows(2).all(|pair| pair[0] < pair[1]) && progress[progress.len() - 1] < lap,
            "{}: checkpoints out of lap order: {progress:?}",
            track.name()
        );
        for (checkpoint, &at) in track.checkpoints().iter().zip(progress) {
            // A checkpoint's progress is that of the centreline point nearest
            // its centre, which lies across the track from it within its
            // reach; height is not part of progress.
            let point = track.point_at(at);
            let off = (point[0] - checkpoint.position[0]).hypot(point[2] - checkpoint.position[2]);
            assert!(
                off <= checkpoint.half_width,
                "{}: checkpoint at {at} m is {off} m off its point",
                track.name()
            );
            assert!((track.progress_at(checkpoint.position) - at).abs() < 1e-3);
        }
        assert_eq!(track.point_at(lap), track.point_at(0.0), "a lap wraps");
        assert_eq!(track.point_at(-1.0), track.point_at(lap - 1.0));
    }
}

#[test]
fn the_spawn_grid_sits_just_behind_the_start_line_facing_it() {
    let track = oval();
    let start = track.checkpoints()[0];
    for slot in track.spawn_grid() {
        let behind = slot.position[2] - start.position[2];
        assert!(
            (1.0..30.0).contains(&behind),
            "slot is {behind} m behind the line"
        );
        assert_eq!(slot.yaw, start.yaw);
    }
}

#[test]
fn a_sidecar_the_sim_cannot_race_on_is_rejected() {
    let other_version = OVAL.replacen("\"version\":1", "\"version\":2", 1);
    assert!(matches!(
        Track::from_sidecar(&other_version),
        Err(TrackError::UnsupportedVersion(2))
    ));

    assert!(matches!(
        Track::from_sidecar(&OVAL[..OVAL.len() / 2]),
        Err(TrackError::Json(_))
    ));

    let no_spawn = OVAL.replacen("\"spawn_grid\":[", "\"spawn_grid\":[],\"unused\":[", 1);
    assert!(matches!(
        Track::from_sidecar(&no_spawn),
        Err(TrackError::Invalid(_))
    ));

    let bad_index = OVAL.replacen("\"indices\":[", "\"indices\":[999999,0,1,", 1);
    assert!(matches!(
        Track::from_sidecar(&bad_index),
        Err(TrackError::Invalid(_))
    ));
}

#[test]
fn brands_patchs_hazard_zone_reaches_as_far_as_the_wheel_can_land_and_its_tyre_beyond() {
    let track = brands_patch();
    assert!(!track.wheel_crossings().is_empty());
    for crossing in track.wheel_crossings() {
        // Short of this, re-export the track after retuning the wheel.
        assert!(
            crossing.hazard_half_depth_m >= WHEEL_LANDING_SPREAD_M + WHEEL_RADIUS_M,
            "{crossing:?}"
        );
    }
}

#[test]
fn brands_patchs_wheel_crossing_reads_its_own_leg_past_the_bridge_not_the_deck_above() {
    // The crossing is 28 m past the bridge on the run to the hairpin,
    // 601.8 m into the lap; the deck it passes under is 1020.7 m in.
    let track = brands_patch();
    let [progress] = track.wheel_crossing_progress() else {
        panic!("one crossing: {:?}", track.wheel_crossings());
    };
    assert!((progress - 601.8).abs() < 1.0, "{progress} m");
}

#[test]
fn a_wheel_crossing_too_wide_for_a_pass_and_its_flag_to_fit_in_the_period_is_rejected() {
    let crossing = |half_width: f32| {
        let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).unwrap();
        sidecar["wheel_crossings"] = serde_json::json!([
            {
                "position": [0.0, 0.0, 0.0],
                "yaw": 0.0,
                "half_width": half_width,
                "hazard_half_depth_m": 9.2
            }
        ]);
        Track::from_sidecar(&sidecar.to_string())
    };
    assert!(crossing(12.0).is_ok(), "Suzuqa Patch's width");
    assert!(matches!(crossing(30.0), Err(TrackError::Invalid(_))));
}

#[test]
fn karts_start_on_the_spawn_grid_in_slot_order() {
    let track = oval();
    let state = started(State::new(SEED, 8, track.clone()));
    let transforms = state.kart_readouts();

    assert_eq!(transforms.len(), 8);
    for (kart, slot) in transforms.iter().zip(track.spawn_grid()) {
        assert_eq!(kart.position[0], slot.position[0]);
        assert_eq!(kart.position[2], slot.position[2]);
        assert!((kart.position[1] - slot.position[1] - KART_HALF_HEIGHT_M).abs() < 1e-5);
        assert_eq!(
            kart.rotation,
            [0.0, 0.0, 0.0, 1.0],
            "the oval grid faces -Z"
        );
    }
}

#[test]
fn a_spawn_slot_yaw_turns_the_kart_counter_clockwise_seen_from_above() {
    // Yaw is radians about +Y, so a quarter turn from -Z faces -X. The
    // exporter and the sim must agree on this or every grid faces wrong.
    let quarter = std::f32::consts::FRAC_PI_2;
    let sidecar = OVAL.replacen(
        "\"spawn_grid\":[{\"position\":[28.0,0.0,4.0],\"yaw\":0.0}",
        &format!("\"spawn_grid\":[{{\"position\":[28.0,0.0,4.0],\"yaw\":{quarter}}}"),
        1,
    );
    assert_ne!(
        sidecar, OVAL,
        "slot 0 of the oval is where this test expects it"
    );
    let mut state = started(State::new(SEED, 1, Track::from_sidecar(&sidecar).unwrap()));
    let spawn = state.kart_readouts()[0];
    for _ in 0..ticks(1.0) {
        state = step(state, &[throttle()], DT);
    }
    let kart = state.kart_readouts()[0];

    assert!(
        kart.position[0] < spawn.position[0] - 1.0,
        "drove toward -X: {kart:?}"
    );
    assert!(
        (kart.position[2] - spawn.position[2]).abs() < 0.1,
        "held its line: {kart:?}"
    );
}

#[test]
#[should_panic(expected = "spawn slots")]
fn more_karts_than_spawn_slots_is_a_caller_error() {
    started(State::new(SEED, 9, oval()));
}

#[test]
fn every_spawn_slot_is_on_asphalt_before_the_first_tick() {
    for track in [oval(), brands_patch()] {
        let state = started(State::new(SEED, 8, track));
        for kart in 0..8 {
            assert_eq!(
                state.kart_surface(kart),
                Surface::Asphalt,
                "{:?} slot {kart}",
                state.track()
            );
        }
    }
}

#[test]
fn the_surface_beside_the_track_is_sand_then_grass_then_nothing() {
    // The oval's front straight: centreline at x = 30, asphalt 6 m either
    // side, sand for 6 m beyond, grass for 40 m beyond the track's bounds.
    let state = started(State::new(SEED, 1, oval()));
    let at = |x: f32| state.surface_at([x, 0.0, 4.0]);

    assert_eq!(at(30.0), Surface::Asphalt);
    assert_eq!(at(39.0), Surface::Sand);
    assert_eq!(at(50.0), Surface::Grass);
    assert_eq!(at(110.0), Surface::OutOfWorld);
    assert_eq!(state.surface_at([0.0, 0.0, 1000.0]), Surface::OutOfWorld);
}

#[test]
fn walls_read_as_wall_and_tyre_walls_as_tyres_from_above() {
    let track = oval();
    let state = started(State::new(SEED, 1, track.clone()));
    let standing: Vec<_> = track
        .surfaces()
        .iter()
        .filter(|mesh| matches!(mesh.surface, Surface::Wall | Surface::Tyres))
        .collect();
    for surface in [Surface::Wall, Surface::Tyres] {
        assert!(
            standing.iter().any(|mesh| mesh.surface == surface),
            "the oval has no {surface:?} mesh"
        );
    }

    for mesh in standing {
        let vertices: Vec<[f32; 3]> = mesh.vertices().collect();
        // A triangle facing up is part of a top; probe above its centre.
        let top = mesh
            .triangles()
            .map(|[a, b, c]| {
                [
                    vertices[a as usize],
                    vertices[b as usize],
                    vertices[c as usize],
                ]
            })
            .find(|[a, b, c]| {
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let normal_y = u[2] * v[0] - u[0] * v[2];
                let area = (u[1] * v[2] - u[2] * v[1])
                    .hypot(normal_y)
                    .hypot(u[0] * v[1] - u[1] * v[0]);
                normal_y.abs() > 0.9 * area
            })
            .unwrap_or_else(|| panic!("{} has no top face", mesh.name));
        let centre = [
            (top[0][0] + top[1][0] + top[2][0]) / 3.0,
            (top[0][1] + top[1][1] + top[2][1]) / 3.0 + 1.0,
            (top[0][2] + top[1][2] + top[2][2]) / 3.0,
        ];
        assert_eq!(state.surface_at(centre), mesh.surface, "{}", mesh.name);
    }
}

#[test]
fn the_run_off_is_walled_on_both_sides_at_every_checkpoint() {
    // Asphalt to 6 m either side of the centreline, sand to 12 m, then the
    // wall: the course is a closed corridor, so no checkpoint can be skipped.
    const ON_THE_SAND_M: f32 = 11.0;
    const IN_THE_WALL_M: f32 = 12.25;
    for track in [oval(), brands_patch()] {
        let state = started(State::new(SEED, 1, track.clone()));
        for (checkpoint, &progress) in track.checkpoints().iter().zip(track.checkpoint_progress()) {
            let [x, y, z] = checkpoint.position;
            let ahead = track.point_at(progress + 1.0);
            let (dx, dz) = (ahead[0] - x, ahead[2] - z);
            let length = dx.hypot(dz);
            let across = |side: f32, m: f32| {
                [
                    x - side * m * dz / length,
                    y + 1.5,
                    z + side * m * dx / length,
                ]
            };
            for side in [1.0, -1.0] {
                let name = track.name();
                assert_eq!(
                    state.surface_at(across(side, ON_THE_SAND_M)),
                    Surface::Sand,
                    "{name} at {x}, {z}"
                );
                assert_eq!(
                    state.surface_at(across(side, IN_THE_WALL_M)),
                    Surface::Wall,
                    "{name} at {x}, {z}"
                );
            }
        }
    }
}

/// Where the oval's first tyre wall stands, seen from above: the (x, z)
/// centre of its footprint and its radius.
fn ovals_first_tyre_wall() -> ([f32; 2], f32) {
    let track = oval();
    let wall = track
        .surfaces()
        .iter()
        .find(|mesh| mesh.name == "TyreWall.00")
        .expect("the oval's first tyre wall");
    let vertices: Vec<[f32; 3]> = wall.vertices().collect();
    let count = vertices.len() as f32;
    let centre = [
        vertices.iter().map(|v| v[0]).sum::<f32>() / count,
        vertices.iter().map(|v| v[2]).sum::<f32>() / count,
    ];
    let radius = (vertices[0][0] - centre[0]).hypot(vertices[0][2] - centre[1]);
    (centre, radius)
}

/// Metres between a kart's centre and the tyre wall's, seen from above.
fn from_the_tyre_wall(kart: &KartReadout, centre: [f32; 2]) -> f32 {
    (kart.position[0] - centre[0]).hypot(kart.position[2] - centre[1])
}

/// The kart's velocity along the ground away from the tyre wall's centre.
fn moving_away_from(kart: &KartReadout, centre: [f32; 2]) -> f32 {
    let (dx, dz) = (kart.position[0] - centre[0], kart.position[2] - centre[1]);
    (kart.velocity[0] * dx + kart.velocity[2] * dz) / dx.hypot(dz)
}

/// A kart's centre comes within this of the tyre wall's once it touches:
/// the wall's radius and the kart's half length, plus a little slack.
fn touching_within(radius: f32) -> f32 {
    radius + KART_HALF_LENGTH_M + 0.1
}

#[test]
fn a_kart_driving_into_a_tyre_wall_bounces_off_it() {
    // Oval slot 0 drives up the front straight, into the first turn and
    // head on into the tyre wall centred on its column.
    let (centre, radius) = ovals_first_tyre_wall();
    let mut state = started(State::new(SEED, 1, oval()));
    let mut touch = None;
    for tick in 0..ticks(9.0) {
        state = step(state, &[throttle()], DT);
        let kart = state.kart_readouts()[0];
        assert!(
            from_the_tyre_wall(&kart, centre) > radius,
            "went into the tyre wall: {kart:?}"
        );
        if touch.is_none() && from_the_tyre_wall(&kart, centre) < touching_within(radius) {
            touch = Some(tick);
        }
        if touch.is_some_and(|touch| tick - touch < ticks(1.0))
            && moving_away_from(&kart, centre) > 1.0
        {
            assert!(
                (kart.position[1] - KART_HALF_HEIGHT_M).abs() < 0.2,
                "left the ground: {kart:?}"
            );
            return;
        }
    }
    panic!(
        "{}",
        match touch {
            None => "never reached the tyre wall".to_owned(),
            Some(_) => format!(
                "still not moving away a second after the touch: {:?}",
                state.kart_readouts()[0]
            ),
        }
    );
}

/// A kart started `lateral` metres right of the oval's first tyre wall's
/// centre and 25 m short of it, driving straight at it from rest until it
/// touches and coasting after: its speed along the ground half a second
/// after the touch, and whether anything spun it out or raised an event.
fn coasting_off_the_tyre_wall(lateral: f32) -> (f32, bool) {
    let (centre, radius) = ovals_first_tyre_wall();
    // Yaw zero faces -Z, toward the wall, and +X is the kart's right.
    let sidecar = with_slot_at(OVAL, 0, [centre[0] + lateral, 0.0, centre[1] + 25.0], 0.0);
    let mut state = started(State::new(SEED, 1, track(&sidecar)));
    let mut touched = None;
    let mut disturbed = false;
    for tick in 0..ticks(6.0) {
        let input = if touched.is_some() {
            KartInput::default()
        } else {
            throttle()
        };
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        disturbed |= kart.spin_out_s.is_some() || !state.events().is_empty();
        if touched.is_none() && from_the_tyre_wall(&kart, centre) < touching_within(radius) {
            touched = Some(tick);
        }
        if touched.is_some_and(|touched| tick - touched == ticks(0.5)) {
            return (kart.velocity[0].hypot(kart.velocity[2]), disturbed);
        }
    }
    panic!("kart {lateral} m right of the tyre wall never touched it");
}

#[test]
fn a_square_hit_on_a_tyre_wall_leaves_slower_than_a_slanting_one() {
    let (square, _) = coasting_off_the_tyre_wall(0.0);
    let (slanting, _) = coasting_off_the_tyre_wall(1.8);
    assert!(
        square < slanting,
        "square left at {square} m/s, slanting at {slanting} m/s"
    );
    assert!(
        square > 1.0,
        "the square hit stopped the kart: {square} m/s"
    );
}

#[test]
fn a_tyre_wall_touch_never_spins_a_kart_out_or_raises_an_event() {
    for lateral in [0.0, 1.8] {
        let (_, disturbed) = coasting_off_the_tyre_wall(lateral);
        assert!(
            !disturbed,
            "the touch {lateral} m right of centre spun out or raised an event"
        );
    }
}

/// Drive `slot` straight ahead for eight seconds into the first turn, whose
/// outer wall rings `turn_centre` (x, z) at `wall_radius`, standing on the
/// outer edge of the sand. Once `in_the_turn` the kart must never cross the
/// wall, and it must end up scraping along it, still on the sand.
fn held_by_the_first_turns_wall(
    track: Track,
    slot: usize,
    turn_centre: [f32; 2],
    wall_radius: f32,
    in_the_turn: impl Fn([f32; 3]) -> bool,
) {
    let radius =
        |position: [f32; 3]| (position[0] - turn_centre[0]).hypot(position[2] - turn_centre[1]);
    let mut state = started(State::new(SEED, slot + 1, track));
    let mut inputs = vec![KartInput::default(); slot + 1];
    inputs[slot] = throttle();
    for _ in 0..ticks(8.0) {
        state = step(state, &inputs, DT);
        let kart = state.kart_readouts()[slot];
        assert!(
            !in_the_turn(kart.position) || radius(kart.position) < wall_radius,
            "went through the wall: {kart:?}"
        );
    }
    let kart = state.kart_readouts()[slot];
    assert!(
        radius(kart.position) > wall_radius - 1.5,
        "never reached the wall: {kart:?}"
    );
    assert!(
        (kart.position[1] - KART_HALF_HEIGHT_M + 0.05).abs() < 0.2,
        "left the ground: {kart:?}"
    );
    assert_eq!(
        state.kart_surface(slot),
        Surface::Sand,
        "pinned to the wall on the sand"
    );
}

#[test]
fn a_kart_driving_into_the_ovals_wall_is_held_by_it() {
    // Slot 1 at x = 32; the first turn is centred on (0, -100) with a 30 m
    // radius, and the wall stands 12 m outside the centreline.
    held_by_the_first_turns_wall(oval(), 1, [0.0, -100.0], 42.0, |position| {
        position[2] < -100.0
    });
}

/// The bump a kart carries as its readout shows it: its velocity along the
/// ground less its speed along the way it faces.
fn bump(kart: &KartReadout) -> [f32; 2] {
    // Yaw zero faces -Z and grows counter-clockwise seen from above.
    let (sin, cos) = yaw(kart.rotation).sin_cos();
    [
        kart.velocity[0] + sin * kart.speed,
        kart.velocity[2] + cos * kart.speed,
    ]
}

#[test]
fn a_kart_driven_into_a_wall_and_a_tyre_wall_at_once_is_held_then_bumped_along_the_wall() {
    // Slot 1 driven straight on meets the first turn's outer wall, 42 m
    // round (0, -100), near (31.8, -125.9). The oval's first tyre wall is
    // moved against that wall a little further round, so the kart's nose
    // meets both in one tick.
    const TURN_CENTRE: [f32; 2] = [0.0, -100.0];
    const WALL_RADIUS: f32 = 42.0;
    let (_, tyre_radius) = ovals_first_tyre_wall();
    let (sin, cos) = (-42.0_f32).to_radians().sin_cos();
    let tyre_centre = [
        TURN_CENTRE[0] + (WALL_RADIUS - tyre_radius) * cos,
        TURN_CENTRE[1] + (WALL_RADIUS - tyre_radius) * sin,
    ];
    let corner = with_mesh_centred_on(OVAL, "TyreWall.00", tyre_centre);
    let mut against_both = started(State::new(SEED, 2, track(&corner)));
    // The same drive with the tyre wall where the oval keeps it, clear of
    // the kart's line: the cost of the wall's hold alone.
    let mut against_the_wall = started(State::new(SEED, 2, oval()));
    let inputs = [KartInput::default(), throttle()];
    let mut touch = None;
    let mut moved_away = false;
    for tick in 0..ticks(8.0) {
        against_both = step(against_both, &inputs, DT);
        against_the_wall = step(against_the_wall, &inputs, DT);
        let kart = against_both.kart_readouts()[1];
        let (dx, dz) = (
            kart.position[0] - TURN_CENTRE[0],
            kart.position[2] - TURN_CENTRE[1],
        );
        let from_the_turn_centre = dx.hypot(dz);
        // The turn starts level with its centre; the front straight runs
        // further out than the wall's radius.
        assert!(
            dz > 0.0 || from_the_turn_centre < WALL_RADIUS,
            "went through the wall: {kart:?}"
        );
        assert!(
            from_the_tyre_wall(&kart, tyre_centre) > tyre_radius,
            "went into the tyre wall: {kart:?}"
        );
        if touch.is_none() && from_the_tyre_wall(&kart, tyre_centre) < touching_within(tyre_radius)
        {
            touch = Some(tick);
            let held_alone = against_the_wall.kart_readouts()[1];
            assert!(
                kart.speed <= held_alone.speed,
                "the wall's hold alone left {} m/s, the corner {} m/s",
                held_alone.speed,
                kart.speed
            );
        }
        if touch.is_some() {
            let [bx, bz] = bump(&kart);
            let into_the_wall = (bx * dx + bz * dz) / from_the_turn_centre;
            assert!(
                into_the_wall < 0.01,
                "bumped {into_the_wall} m/s into the wall: {kart:?}"
            );
        }
        moved_away |= touch.is_some_and(|touch| tick - touch < ticks(1.0))
            && moving_away_from(&kart, tyre_centre) > 1.0;
    }
    assert!(touch.is_some(), "never reached the corner");
    assert!(moved_away, "never moved away from the tyre wall");
}

#[test]
fn a_kart_driving_into_a_brands_patch_wall_is_held_by_it() {
    // The start straight runs 60 m from the line at the origin along
    // (0.643, 0.766) into the first curve, a 50 m right-hander centred on
    // (0.3, 78.2). Driven straight on, slot 0 meets the wall 12 m outside
    // the centreline.
    let curve_start = [38.6, 46.0];
    held_by_the_first_turns_wall(brands_patch(), 0, [0.3, 78.2], 62.0, |position| {
        (position[0] - curve_start[0]) * 0.643 + (position[2] - curve_start[1]) * 0.766 > 0.0
    });
}

#[test]
fn a_kart_drives_brands_patch_without_falling_through() {
    let mut state = started(State::new(SEED, 1, brands_patch()));
    let spawn = state.kart_readouts()[0];
    for _ in 0..ticks(3.0) {
        state = step(state, &[throttle()], DT);
    }
    let kart = state.kart_readouts()[0];
    let [dx, dz] = state.track().direction_at(0.0);

    assert!(
        (kart.position[0] - spawn.position[0]) * dx + (kart.position[2] - spawn.position[2]) * dz
            > 30.0,
        "drove up the straight: {kart:?}"
    );
    assert!(
        (kart.position[1] - KART_HALF_HEIGHT_M).abs() < 0.1,
        "fell or flew: {kart:?}"
    );
    assert_eq!(state.kart_surface(0), Surface::Asphalt);
}

#[test]
fn direction_and_curvature_follow_the_centreline_with_a_left_turn_positive() {
    let track = oval();
    // The front straight runs along -Z from the line at x = 30; the first
    // turn is a 30 m left-hander centred on (0, -100).
    let [dx, dz] = track.direction_at(50.0);
    assert!(
        dx.abs() < 0.01 && (dz + 1.0).abs() < 0.01,
        "front straight: [{dx}, {dz}]"
    );
    assert!(
        track.curvature_at(50.0).abs() < 1e-3,
        "a straight has no curvature"
    );

    let apex = 100.0 + std::f32::consts::PI * 30.0 / 2.0;
    let point = track.point_at(apex);
    assert!(
        point[0].abs() < 1.0 && (point[2] + 130.0).abs() < 1.0,
        "apex at {point:?}"
    );
    let [dx, dz] = track.direction_at(apex);
    assert!(
        (dx + 1.0).abs() < 0.05 && dz.abs() < 0.05,
        "heading west: [{dx}, {dz}]"
    );
    let curvature = track.curvature_at(apex);
    assert!(
        (curvature - 1.0 / 30.0).abs() < 0.003,
        "a 30 m left turn reads +1/30 per metre, not {curvature}"
    );

    // Suzuqa Patch's chicane turns in right on a 12 m arc, its tightest.
    let brands = brands_patch();
    let sharpest_right = (0..brands.spline_length() as u32)
        .map(|at| brands.curvature_at(at as f32))
        .fold(0.0_f32, f32::min);
    assert!(
        (sharpest_right + 1.0 / 12.0).abs() < 0.01,
        "a right turn reads negative: {sharpest_right}"
    );
}
