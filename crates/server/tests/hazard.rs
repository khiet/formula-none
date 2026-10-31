//! The hazard over the fake transport (ADR 0009): a hit is decided by the
//! server alone and every client is told the same thing, the Hit event on
//! the reliable lane and the spin-out timer in the snapshots. Two humans
//! race the oval's line among the bots: the first trails the first hazard
//! it rolls and parks on the line, the second follows a second behind, and
//! whoever is hit first, by whoever's hazard, is the hit both clients see.

mod common;

use std::time::Duration;

use common::{follow_the_line, race_together, seconds, speed, wire, Heard};
use protocol::{Event, EventKind, Item, ItemSlot, Snapshot};
use server::fake_transport::{Conditions, Harness};
use sim::tuning::{COUNTDOWN_S, SPIN_OUT_S, TICKS_PER_SECOND};
use sim::{KartInput, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const TICK: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);
/// Give up if nobody has been hit by then.
const DEADLINE_S: f32 = 90.0;

impl Heard {
    fn hit(&self) -> Option<Event> {
        self.events
            .iter()
            .copied()
            .find(|event| matches!(event.kind, EventKind::Hit { .. }))
    }

    /// The first snapshot showing `kart` spun out.
    fn spun(&self, kart: u8) -> Option<&Snapshot> {
        self.snapshots
            .values()
            .find(|snapshot| snapshot.karts[usize::from(kart)].spin_out_s.is_some())
    }

    /// Whether the hit has arrived on both lanes: the event, and then the
    /// snapshot that shows its spin-out.
    fn heard_the_hit(&self) -> bool {
        self.hit().is_some_and(|hit| {
            let EventKind::Hit { kart, .. } = hit.kind else {
                unreachable!()
            };
            self.spun(kart).is_some()
        })
    }
}

#[tokio::test(start_paused = true)]
async fn a_hit_is_the_servers_call_and_both_clients_see_the_same_hit_and_spin_out() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = [harness.connect(), harness.connect()];
    let countdown = race_together(&code, &mut players).await.remove(0);
    assert_eq!(countdown.grid, vec![0, 1]);
    let track = Track::from_sidecar(OVAL).expect("the oval fixture loads");

    let mut heard = [Heard::default(), Heard::default()];
    let mut sequences = [0u32; 2];
    // Whether the dropper has fired its hazard, which it trails from then
    // on, parked on the line.
    let mut trailing = false;
    let mut ticks = tokio::time::interval(TICK);
    let deadline = seconds(0) + DEADLINE_S;
    loop {
        ticks.tick().await;
        for (player, heard) in players.iter_mut().zip(&mut heard) {
            heard.hear(player);
            assert!(heard.results.is_none(), "the race ended with nobody hit");
        }
        if heard.iter().all(Heard::heard_the_hit) {
            break;
        }
        let Some(newest) = heard[0].newest() else {
            continue;
        };
        let elapsed = seconds(newest.tick);
        assert!(elapsed < deadline, "nobody was hit in {DEADLINE_S} s");
        if elapsed < COUNTDOWN_S {
            continue;
        }
        trailing |= heard[0].events.iter().any(|event| {
            event.kind
                == EventKind::ItemFired {
                    kart: 0,
                    item: Item::Hazard,
                }
        });

        // The dropper: drives the line firing whatever it rolls, with the
        // button held so a hazard trails; once it trails one it brakes to a
        // stop and holds there, throttle and brake together.
        let dropper = &newest.karts[0];
        let mut input = if trailing {
            KartInput {
                throttle: f32::from(u8::from(speed(newest, 0) < 1.0)),
                brake: 1.0,
                ..KartInput::default()
            }
        } else {
            follow_the_line(&track, dropper)
        };
        input.use_item = matches!(dropper.item_slot, ItemSlot::Held(_));
        input.hold_item = true;
        sequences[0] += 1;
        players[0].send_input(wire(sequences[0], input));

        // The follower: the same line, a second behind.
        if elapsed >= COUNTDOWN_S + 1.0 {
            let input = follow_the_line(&track, &newest.karts[1]);
            sequences[1] += 1;
            players[1].send_input(wire(sequences[1], input));
        }
    }

    // The same event, under the same id, on both reliable lanes.
    let hit = heard[0].hit().unwrap();
    assert_eq!(heard[1].hit(), Some(hit));
    let EventKind::Hit { kart, by: Some(by) } = hit.kind else {
        panic!("a hit by a kart's hazard: {hit:?}")
    };
    assert_ne!(kart, by, "nobody hits their own hazard this early");

    // Both clients saw the hazard on the track before the hit, and both see
    // the same spin-out in the same snapshot afterwards.
    for heard in &heard {
        assert!(
            heard
                .snapshots
                .values()
                .any(|snapshot| snapshot.hazards.iter().any(|hazard| hazard.owner == by)),
            "the hazard rode the snapshots"
        );
    }
    let (first, second) = (heard[0].spun(kart).unwrap(), heard[1].spun(kart).unwrap());
    assert_eq!(first.tick, second.tick);
    assert_eq!(first.karts, second.karts, "the same state on both clients");
    let spun_kart = &first.karts[usize::from(kart)];
    assert_eq!(speed(first, usize::from(kart)), 0.0, "stopped");
    let left = spun_kart.spin_out_s.unwrap();
    assert!(
        left > 0.0 && left <= SPIN_OUT_S,
        "{left} s of spin-out left"
    );
}
