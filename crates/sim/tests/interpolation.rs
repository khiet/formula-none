//! What the interpolation draws between the two snapshots either side of
//! its render clock: projectiles matched across them by owner and target,
//! held where the earlier snapshot had one the later no longer carries,
//! and not drawn before the snapshot that first carries one; a kart moved
//! further than a kart drives, which is drawn in one move; and a kart's
//! boost, as the earlier snapshot has it.

use sim::tuning::{INTERPOLATION_TELEPORT_M_PER_TICK, SNAPSHOTS_PER_SECOND, TICKS_PER_SECOND};
use sim::{Interpolation, KartReadout, MiniTurbo, Projectile, Start};

const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;

fn projectile(x: f32, owner: usize, target: usize) -> Projectile {
    Projectile {
        position: [x, 0.0, 0.0],
        owner,
        target,
    }
}

/// A kart standing at `x` as a snapshot carries it.
fn kart_at(x: f32) -> KartReadout {
    KartReadout {
        position: [x, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        speed: 0.0,
        velocity: [0.0; 3],
        bump: None,
        drift: None,
        mini_turbo: MiniTurbo::None,
        held_item: None,
        tyres: 0,
        spin_out_s: None,
        airborne: false,
        boost_s: None,
        boost_tier: MiniTurbo::None,
        start: Start::Clean,
        stall_s: None,
        race_position: 1,
        lap: 1,
        checkpoint: 0,
        spline_progress: 0.0,
        wrong_way: false,
    }
}

/// Snapshots at the snapshot rate from tick 0, `count` of them, each with
/// `at(tick)`'s projectiles, pushed as they would arrive, and the clock run
/// on until it draws between the second and third.
fn drawing_between_the_second_and_third(
    count: u32,
    at: impl Fn(u32) -> Vec<Projectile>,
) -> Interpolation {
    drawing_snapshots_between_the_second_and_third(count, |tick| (Vec::new(), at(tick)))
}

/// As above, with `at(tick)`'s karts and projectiles.
fn drawing_snapshots_between_the_second_and_third(
    count: u32,
    at: impl Fn(u32) -> (Vec<KartReadout>, Vec<Projectile>),
) -> Interpolation {
    let mut interpolation = Interpolation::new();
    for snapshot in 0..count {
        let tick = snapshot * TICKS_PER_SNAPSHOT;
        let (karts, projectiles) = at(tick);
        interpolation.push(tick, karts, projectiles);
    }
    while interpolation
        .render_tick()
        .is_none_or(|render| render < TICKS_PER_SNAPSHOT as f32 + 0.5)
    {
        interpolation.advance();
    }
    interpolation
}

#[test]
fn a_projectile_is_drawn_between_the_snapshots_either_side_of_the_render_clock() {
    // The one projectile flies +X a metre a snapshot.
    let interpolation =
        drawing_between_the_second_and_third(6, |tick| vec![projectile(tick as f32, 3, 2)]);
    let render = interpolation.render_tick().unwrap();
    assert!(render > TICKS_PER_SNAPSHOT as f32 && render < 2.0 * TICKS_PER_SNAPSHOT as f32);
    let [drawn] = interpolation.projectiles()[..] else {
        panic!("one projectile: {:?}", interpolation.projectiles());
    };
    assert_eq!((drawn.owner, drawn.target), (3, 2));
    assert!(
        (drawn.position[0] - render).abs() < 1e-4,
        "drawn at x {} for a clock at {render}",
        drawn.position[0]
    );
}

#[test]
fn a_projectile_gone_from_the_later_snapshot_stays_where_the_earlier_had_it() {
    // Kart 1's projectile reaches its target between the second and third
    // snapshots; kart 5's, fired between them, flies on.
    let interpolation = drawing_between_the_second_and_third(6, |tick| {
        let mut flying = Vec::new();
        if tick < 2 * TICKS_PER_SNAPSHOT {
            flying.push(projectile(tick as f32, 1, 0));
        }
        if tick >= 2 * TICKS_PER_SNAPSHOT {
            flying.push(projectile(100.0 + tick as f32, 5, 4));
        }
        flying
    });
    let [drawn] = interpolation.projectiles()[..] else {
        panic!(
            "only the one the earlier snapshot carries: {:?}",
            interpolation.projectiles()
        );
    };
    assert_eq!(drawn, projectile(TICKS_PER_SNAPSHOT as f32, 1, 0));
}

#[test]
fn projectiles_are_matched_across_snapshots_in_fired_order() {
    // Two projectiles of the same owner at the same target: the first in
    // fired order follows the first, whatever their positions.
    let interpolation = drawing_between_the_second_and_third(6, |tick| {
        vec![
            projectile(tick as f32, 2, 1),
            projectile(50.0 + tick as f32, 2, 1),
        ]
    });
    let render = interpolation.render_tick().unwrap();
    let drawn = interpolation.projectiles();
    assert_eq!(drawn.len(), 2);
    assert!((drawn[0].position[0] - render).abs() < 1e-4);
    assert!((drawn[1].position[0] - (50.0 + render)).abs() < 1e-4);
}

#[test]
fn nothing_is_drawn_before_the_first_snapshot() {
    let mut interpolation = Interpolation::new();
    assert!(interpolation.projectiles().is_empty());
    interpolation.advance();
    assert!(interpolation.projectiles().is_empty());
}

#[test]
fn a_kart_put_somewhere_new_is_drawn_at_the_old_place_until_the_snapshot_that_moved_it() {
    // From the third snapshot on, the kart is further along than a kart
    // drives in a snapshot's ticks: a respawn. Drawn between the second and
    // third, it is still where the second had it, never part way across.
    let far = TICKS_PER_SNAPSHOT as f32 * INTERPOLATION_TELEPORT_M_PER_TICK + 1.0;
    let interpolation = drawing_snapshots_between_the_second_and_third(6, |tick| {
        let x = if tick < 2 * TICKS_PER_SNAPSHOT {
            0.0
        } else {
            far
        };
        (vec![kart_at(x)], Vec::new())
    });
    let [drawn] = interpolation.kart_readouts()[..] else {
        panic!("one kart: {:?}", interpolation.kart_readouts());
    };
    assert_eq!(
        drawn.position[0], 0.0,
        "drawn where the earlier snapshot had it"
    );

    // A kart that drove the same way a metre a snapshot is drawn between.
    let interpolation = drawing_snapshots_between_the_second_and_third(6, |tick| {
        (
            vec![kart_at(tick as f32 / TICKS_PER_SNAPSHOT as f32)],
            Vec::new(),
        )
    });
    let render = interpolation.render_tick().unwrap();
    let drawn = interpolation.kart_readouts()[0];
    let expected = render / TICKS_PER_SNAPSHOT as f32;
    assert!(
        (drawn.position[0] - expected).abs() < 1e-4,
        "drawn at x {} for a clock at {render}",
        drawn.position[0]
    );
}

#[test]
fn a_kart_boosting_in_the_earlier_snapshot_is_drawn_boosting_in_its_tier() {
    let interpolation = drawing_snapshots_between_the_second_and_third(6, |tick| {
        let boost_s = 1.0 - tick as f32 / TICKS_PER_SECOND as f32;
        let kart = KartReadout {
            boost_s: Some(boost_s),
            boost_tier: MiniTurbo::Orange,
            ..kart_at(0.0)
        };
        (vec![kart], Vec::new())
    });
    let drawn = interpolation.kart_readouts()[0];
    let earlier = 1.0 - TICKS_PER_SNAPSHOT as f32 / TICKS_PER_SECOND as f32;
    assert_eq!(drawn.boost_s, Some(earlier));
    assert_eq!(drawn.boost_tier, MiniTurbo::Orange);
}
