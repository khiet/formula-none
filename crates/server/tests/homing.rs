//! The homing projectile over the fake transport (ADR 0009): the fire, the
//! flight and the hit are the server's alone and every client is told the
//! same thing: the fire and the hit on the reliable lane, the projectile's
//! flight and the spin-out in the snapshots. Two humans race the oval's
//! line among the bots: the first drives off at Go, the second waits on the
//! grid for the bots to pass and then chases them, firing whatever it rolls
//! from the back of the field, until a projectile of its reaches the kart
//! ahead of it.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::{
    assert_no_bot_rocket_starts, follow_the_line, race_together, seconds, wire, with_rocket_start,
    Heard,
};
use protocol::{Event, EventKind, Item, ProjectileSnapshot, Snapshot};
use server::fake_transport::{Conditions, Harness};
use sim::tuning::{COUNTDOWN_S, SPIN_OUT_S, TICKS_PER_SECOND};
use sim::{Phase, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);
/// Give up if the chaser's projectile has hit nobody by then.
const DEADLINE_S: f32 = 120.0;
/// How long the chaser waits on the grid for the bots to pass it.
const WAIT_ON_THE_GRID_S: f32 = 6.0;
/// The chaser's kart on the grid.
const CHASER: u8 = 1;

impl Heard {
    /// The first hit the chaser scored.
    fn hit(&self) -> Option<Event> {
        self.events
            .iter()
            .copied()
            .find(|event| matches!(event.kind, EventKind::Hit { by, .. } if by == CHASER))
    }

    /// The chaser's last fire of a projectile before `hit`: the one that
    /// scored it.
    fn fire_before(&self, hit: Event) -> Option<Event> {
        self.events.iter().copied().rfind(|event| {
            event.id < hit.id
                && event.kind
                    == EventKind::ItemFired {
                        kart: CHASER,
                        item: Item::Homing,
                    }
        })
    }

    /// The first snapshot that carries `event`: the state in its aftermath.
    fn snapshot_carrying(&self, event: Event) -> Option<&Snapshot> {
        self.snapshots
            .values()
            .find(|snapshot| snapshot.events.contains(&event))
    }

    /// The chaser's projectiles chasing `kart`, by the tick of the snapshot
    /// carrying them.
    fn flights_at(&self, kart: u8) -> BTreeMap<u32, Vec<ProjectileSnapshot>> {
        self.snapshots
            .iter()
            .filter_map(|(&tick, snapshot)| {
                let flying: Vec<ProjectileSnapshot> = snapshot
                    .projectiles
                    .iter()
                    .copied()
                    .filter(|projectile| projectile.owner == CHASER && projectile.target == kart)
                    .collect();
                (!flying.is_empty()).then_some((tick, flying))
            })
            .collect()
    }

    /// Whether a hit of the chaser's has arrived on both lanes: the event,
    /// and then the snapshot that carries it and shows the spin-out.
    fn heard_the_hit(&self) -> bool {
        self.hit()
            .is_some_and(|hit| self.snapshot_carrying(hit).is_some())
    }
}

#[tokio::test(start_paused = true)]
async fn a_projectiles_fire_flight_and_hit_are_the_servers_and_both_clients_see_the_same() {
    // A seed whose race rolls no bot a rocket start, so the bots pass the
    // waiting chaser clean and it chases from the back of the field.
    let mut harness = Harness::new(Conditions {
        seed: 2,
        ..Conditions::PERFECT
    });
    let code = harness.create_room();
    let mut players = [harness.connect(), harness.connect()];
    let countdown = race_together(&code, &mut players).await.remove(0);
    assert_eq!(countdown.grid, vec![0, CHASER]);
    let track = Track::from_sidecar(OVAL).expect("the oval fixture loads");

    let mut heard = [Heard::default(), Heard::default()];
    let mut sequences = [0u32; 2];
    let mut ticks = tokio::time::interval(TICK);
    loop {
        ticks.tick().await;
        for (player, heard) in players.iter_mut().zip(&mut heard) {
            heard.hear(player);
            assert!(
                heard.results.is_none(),
                "the race ended before the chaser's projectile hit"
            );
        }
        if heard.iter().all(Heard::heard_the_hit) {
            break;
        }
        let Some(newest) = heard[0].newest() else {
            continue;
        };
        let elapsed = seconds(newest.tick);
        assert!(
            elapsed < DEADLINE_S,
            "no hit of the chaser's in {DEADLINE_S} s"
        );
        assert_no_bot_rocket_starts(newest, 2);
        // The leader rocket-starts as a bot times it, then drives the line
        // from Go, holding whatever it rolls.
        let input = with_rocket_start(
            Phase::Countdown {
                remaining_s: COUNTDOWN_S - elapsed,
            },
            follow_the_line(&track, &newest.karts[0]),
        );
        if elapsed < COUNTDOWN_S && input.throttle == 0.0 {
            continue;
        }
        sequences[0] += 1;
        players[0].send_input(wire(sequences[0], input));
        if elapsed < COUNTDOWN_S {
            continue;
        }

        // The chaser waits on the grid, then drives the line firing
        // whatever it rolls the moment it has it.
        if elapsed >= COUNTDOWN_S + WAIT_ON_THE_GRID_S {
            let chaser = &newest.karts[usize::from(CHASER)];
            let mut input = follow_the_line(&track, chaser);
            input.use_item = chaser.held_item.is_some();
            sequences[1] += 1;
            players[1].send_input(wire(sequences[1], input));
        }
    }

    // The same hit and the fire that scored it, under the same ids, on both
    // reliable lanes.
    let hit = heard[0].hit().unwrap();
    let fire = heard[0].fire_before(hit).expect("a fire before the hit");
    assert_eq!(heard[1].hit(), Some(hit));
    assert_eq!(heard[1].fire_before(hit), Some(fire));
    let EventKind::Hit { kart, by } = hit.kind else {
        unreachable!()
    };
    assert_eq!(by, CHASER);
    assert_ne!(kart, CHASER, "a projectile never hits its owner");

    // Both see the same spin-out in the snapshot that carries the hit.
    let (first, second) = (
        heard[0].snapshot_carrying(hit).unwrap(),
        heard[1].snapshot_carrying(hit).unwrap(),
    );
    assert_eq!(first.tick, second.tick);
    assert_eq!(first.karts, second.karts, "the same state on both clients");
    let left = first.karts[usize::from(kart)]
        .spin_out_s
        .expect("spun out by the hit");
    assert!(
        left > 0.0 && left <= SPIN_OUT_S,
        "{left} s of spin-out left"
    );

    // Both clients watched the same flight: from the snapshot carrying the
    // fire to the one before the hit the projectile rode the snapshots,
    // at the same place at every tick both received, and it is gone from
    // the one carrying the hit.
    let fired_at = heard[0].snapshot_carrying(fire).unwrap().tick;
    let hit_at = first.tick;
    let flights = [heard[0].flights_at(kart), heard[1].flights_at(kart)];
    let in_flight: Vec<u32> = flights[0]
        .keys()
        .copied()
        .filter(|&tick| fired_at <= tick && tick < hit_at)
        .collect();
    assert!(!in_flight.is_empty(), "the projectile rode the snapshots");
    assert_eq!(
        in_flight,
        heard[0]
            .snapshots
            .range(fired_at..hit_at)
            .map(|(&tick, _)| tick)
            .collect::<Vec<u32>>(),
        "in every snapshot between the fire and the hit"
    );
    for tick in &in_flight {
        if let Some(theirs) = flights[1].get(tick) {
            assert_eq!(
                &flights[0][tick], theirs,
                "the same projectile at tick {tick}"
            );
        }
    }
    assert!(!flights[0].contains_key(&hit_at), "gone once it hit");
}
