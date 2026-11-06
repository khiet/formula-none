//! The server-authoritative race (ADR 0009, ADR 0010, ADR 0011), driven
//! through the fake transport: the countdown, the snapshots and what they
//! acknowledge, the inputs that reach the sim however the datagrams
//! arrive, the events on both lanes, the race's end, its results, the
//! rematch and the join between races. Under tokio's paused clock a whole
//! race costs only the CPU time of its ticks.

mod common;

use std::collections::BTreeSet;
use std::time::Duration;

use common::{drive_to_results, seconds, speed, Raced};
use protocol::{
    ClientMessage, Event, EventKind, Input, PlayerState, RoomState, ServerMessage, Snapshot,
};
use server::fake_transport::{Client, Conditions, Harness, TRACK};
use server::room::Started;
use sim::tuning::{
    BOT_START_HOLD_S, COUNTDOWN_S, INPUT_QUEUE_MAX_TICKS, INPUT_QUEUE_START_TICKS, KARTS_PER_RACE,
    RACE_OUTRO_S, SNAPSHOTS_PER_SECOND, TICKS_PER_SECOND,
};
use sim::ItemSlot;

const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;
/// The sidecar of the track the harness races, for what the track holds.
const OVAL_SIDECAR: &str = include_str!("../../../assets/tracks/oval/oval.track.json");

/// A room with `count` players in it, in join order, each holding its slot.
async fn room_of(harness: &mut Harness, code: &str, count: usize) -> Vec<Client> {
    let mut clients = Vec::new();
    for index in 0..count {
        let mut client = harness.connect();
        let (slot, _) = client.join(code, &format!("player{index}")).await.unwrap();
        assert_eq!(usize::from(slot), index);
        clients.push(client);
    }
    // Every earlier client has been told about every later join.
    for (index, client) in clients.iter_mut().enumerate() {
        for _ in index + 1..count {
            client.next_room().await.unwrap();
        }
    }
    clients
}

/// Everyone readies, the creator starts, and every client's Countdown is
/// handed back in slot order.
async fn start_race(players: &mut [Client]) -> Vec<Started> {
    for player in players.iter() {
        player.send(&ClientMessage::Ready { ready: true });
    }
    let count = players.len();
    for player in players.iter_mut() {
        let mut last = None;
        for _ in 0..count {
            last = player.next_room().await;
        }
        assert!(last
            .unwrap()
            .players
            .iter()
            .all(|player| player.state == PlayerState::Ready));
    }
    players[0].send(&ClientMessage::Start);
    let mut countdowns = Vec::new();
    for player in players.iter_mut() {
        countdowns.push(player.next_countdown().await.unwrap());
        // The room follows the race into Countdown, with everyone Racing.
        let room = player.next_room().await.unwrap();
        assert_eq!(room.state, RoomState::Countdown);
        assert!(room
            .players
            .iter()
            .all(|player| player.state == PlayerState::Racing));
    }
    countdowns
}

/// A room of `count` players already racing, and their Countdowns.
async fn racing_room(harness: &mut Harness, count: usize) -> (String, Vec<Client>, Vec<Started>) {
    let code = harness.create_room();
    let mut players = room_of(harness, &code, count).await;
    let countdowns = start_race(&mut players).await;
    (code, players, countdowns)
}

fn input(sequence: u32, throttle: u8) -> Input {
    Input {
        sequence,
        steer: 0,
        throttle,
        brake: 0,
        hop: false,
        use_item: false,
        hold_item: false,
    }
}

#[tokio::test(start_paused = true)]
async fn the_countdown_reaches_every_client_with_the_grid_in_room_order_and_the_room_follows_the_race(
) {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = room_of(&mut harness, &code, 3).await;
    let countdowns = start_race(&mut players).await;
    let counting_from = tokio::time::Instant::now();
    for countdown in &countdowns {
        assert_eq!(
            countdown, &countdowns[0],
            "every client builds the same race"
        );
    }
    assert_eq!(countdowns[0].track, TRACK);
    assert_eq!(countdowns[0].grid, vec![0, 1, 2], "humans in room order");

    // Snapshots carry the whole grid: the humans and the bots behind them.
    let snapshot = players[1].next_snapshot().await.unwrap();
    assert_eq!(snapshot.karts.len(), KARTS_PER_RACE);
    assert_eq!(snapshot.acknowledged.len(), KARTS_PER_RACE);
    assert_eq!(snapshot.tick, TICKS_PER_SNAPSHOT, "the first snapshot");

    // The room follows the race into Racing at Go, after the snapshot of
    // the tick that went.
    let room = players[0].next_room().await.unwrap();
    assert_eq!(room.state, RoomState::Racing);
    assert_eq!(
        harness.rooms().view(&code).unwrap().state,
        RoomState::Racing
    );
    // Five start lights, one a second: the room's Countdown lasts 5 s.
    let countdown = counting_from.elapsed().as_secs_f32();
    assert!(
        (countdown - 5.0).abs() <= 0.1,
        "the room's Countdown lasted {countdown} s"
    );
    let snapshot = players[0].next_snapshot().await.unwrap();
    assert!(
        seconds(snapshot.tick) >= COUNTDOWN_S,
        "past Go: tick {}",
        snapshot.tick
    );
}

#[tokio::test(start_paused = true)]
async fn snapshots_come_at_the_snapshot_rate_in_tick_order_and_acknowledge_one_sequence_a_tick() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];

    let first = player.next_snapshot().await.unwrap();
    assert_eq!(first.tick, TICKS_PER_SNAPSHOT);
    assert_eq!(first.acknowledged[0], 0, "nothing sent yet");
    let mut last_tick = first.tick;
    for _ in 0..5 {
        let snapshot = player.next_snapshot().await.unwrap();
        assert_eq!(snapshot.tick, last_tick + TICKS_PER_SNAPSHOT);
        last_tick = snapshot.tick;
    }

    // Four inputs at once are applied one a tick, in order; past the
    // newest the server runs on the last controls for the tuned depth, in
    // case the next is lost, then waits for the client.
    for sequence in 1..=4 {
        player.send_input(input(sequence, 0));
    }
    let held = 4 + INPUT_QUEUE_MAX_TICKS;
    let mut acknowledged = 0;
    while acknowledged < held {
        let snapshot = player.next_snapshot().await.unwrap();
        let next = snapshot.acknowledged[0];
        assert!(
            next >= acknowledged && next - acknowledged <= TICKS_PER_SNAPSHOT,
            "acknowledged {next} after {acknowledged}"
        );
        acknowledged = next;
    }
    // An older sequence, or a repeat, changes nothing.
    player.send_input(input(3, 0));
    player.send_input(input(4, 0));
    let snapshot = player.next_snapshot().await.unwrap();
    assert_eq!(snapshot.acknowledged[0], held, "never back, never past");
    assert!(
        snapshot.acknowledged[1..]
            .iter()
            .all(|&sequence| sequence == 0),
        "a bot's kart acknowledges nothing"
    );
}

#[tokio::test(start_paused = true)]
async fn a_client_far_ahead_is_caught_up_to_and_one_back_from_a_stall_is_heard_again() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];
    while seconds(player.next_snapshot().await.unwrap().tick) < COUNTDOWN_S + 0.5 {}

    // Ten inputs at once: the server starts no further than the tuned depth
    // behind the newest, dropping the oldest.
    for sequence in 1..=10 {
        player.send_input(input(sequence, 0));
    }
    let first = player.next_snapshot().await.unwrap().acknowledged[0];
    assert!(first >= 10 - INPUT_QUEUE_MAX_TICKS, "acknowledged {first}");

    // Then the client stalls: the server runs the tuned depth past the
    // newest and waits there, and the client's next inputs drive the kart.
    let held = 10 + INPUT_QUEUE_MAX_TICKS;
    while player.next_snapshot().await.unwrap().acknowledged[0] < held {}
    assert_eq!(player.next_snapshot().await.unwrap().acknowledged[0], held);
    for sequence in held + 1..=held + 2 {
        player.send_input(input(sequence, u8::MAX));
    }
    let mut snapshot = player.next_snapshot().await.unwrap();
    while seconds(snapshot.tick) < COUNTDOWN_S + 2.5 {
        snapshot = player.next_snapshot().await.unwrap();
    }
    assert!(snapshot.acknowledged[0] > held);
    assert!(
        speed(&snapshot, 0) > 5.0,
        "driving: {:?}",
        snapshot.karts[0]
    );
}

#[tokio::test(start_paused = true)]
async fn inputs_that_arrive_out_of_order_are_applied_in_sequence_order() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];
    // Past Go, so the throttle moves the kart.
    while seconds(player.next_snapshot().await.unwrap().tick) < COUNTDOWN_S + 0.5 {}

    // Sequence 3 says go; 1 and 2, overtaken on the way, say stop. They are
    // applied first, and the kart goes.
    player.send_input(input(3, u8::MAX));
    player.send_input(input(2, 0));
    player.send_input(input(1, 0));
    let mut snapshot = player.next_snapshot().await.unwrap();
    while seconds(snapshot.tick) < COUNTDOWN_S + 2.0 {
        snapshot = player.next_snapshot().await.unwrap();
    }
    assert!(snapshot.acknowledged[0] >= 3);
    assert!(
        speed(&snapshot, 0) > 5.0,
        "driving: {:?}",
        snapshot.karts[0]
    );
}

#[tokio::test(start_paused = true)]
async fn inputs_survive_a_lossy_jittery_link_and_the_acknowledgement_never_goes_backwards() {
    let mut harness = Harness::new(Conditions {
        delay: Duration::from_millis(40),
        jitter: Duration::from_millis(25),
        loss: 0.3,
        seed: 11,
    });
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];

    // The client sends the throttle with a fresh sequence on every snapshot
    // it sees, as the browser does every tick.
    let mut sequence = 0;
    let mut acknowledged = 0;
    let mut snapshots = 0;
    let mut last_tick = 0;
    let mut moving = false;
    let mut reordered = 0;
    while !moving || snapshots < 60 {
        let snapshot = player.next_snapshot().await.unwrap();
        snapshots += 1;
        // A snapshot the jitter let a later one overtake is stale: the
        // client keeps the newest tick it has seen.
        if snapshot.tick <= last_tick {
            reordered += 1;
            continue;
        }
        last_tick = snapshot.tick;
        assert!(
            snapshot.acknowledged[0] >= acknowledged,
            "acknowledged {} after {acknowledged}",
            snapshot.acknowledged[0]
        );
        acknowledged = snapshot.acknowledged[0];
        // A lost input's tick still uses up its sequence, so the server may
        // run the tuned depth past the newest it has.
        assert!(
            acknowledged <= sequence + INPUT_QUEUE_MAX_TICKS,
            "acknowledged {acknowledged} with {sequence} sent"
        );
        if seconds(snapshot.tick) > COUNTDOWN_S + 3.0 && speed(&snapshot, 0) > 5.0 {
            moving = true;
        }
        sequence += 1;
        player.send_input(input(sequence, u8::MAX));
        assert!(snapshots < 2000, "the kart never got going");
    }
    assert!(acknowledged > 0, "inputs got through the loss");
    assert!(reordered > 0, "the jitter reordered some snapshots");
}

/// The next message that is an Events message or a Snapshot.
async fn next_events_or_snapshot(client: &mut Client) -> Option<Result<Vec<Event>, Snapshot>> {
    client
        .next_where(|message| match message {
            ServerMessage::Events(events) => Some(Ok(events)),
            ServerMessage::Snapshot(snapshot) => Some(Err(snapshot)),
            _ => None,
        })
        .await
}

#[tokio::test(start_paused = true)]
async fn an_event_comes_once_on_the_reliable_lane_and_in_three_snapshots_under_its_id() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];

    // The first event is a bot's, behind the idle human: a coin taken on the
    // front straight.
    let first = loop {
        match next_events_or_snapshot(player).await.unwrap() {
            Ok(events) => break events,
            Err(snapshot) => assert!(snapshot.events.is_empty(), "no event before the first"),
        }
    };
    let event = first[0];
    assert!(
        matches!(event.kind, EventKind::CoinPicked { kart, .. } if kart >= 1),
        "{event:?}"
    );

    // Every snapshot from here repeats it, until it has been in three.
    let mut carried = 0;
    let mut seen_on_the_stream = 1;
    loop {
        match next_events_or_snapshot(player).await.unwrap() {
            Ok(events) => {
                for later in events {
                    assert!(later.id > event.id, "ids count up: {later:?}");
                    seen_on_the_stream += usize::from(later.id == event.id);
                }
            }
            Err(snapshot) => {
                let repeats = snapshot
                    .events
                    .iter()
                    .filter(|repeat| repeat.id == event.id)
                    .count();
                if repeats == 0 {
                    break;
                }
                assert_eq!(repeats, 1, "once per snapshot");
                assert!(snapshot.events.contains(&event), "the same event");
                carried += 1;
            }
        }
        assert!(carried <= 3, "repeated in more than three snapshots");
    }
    assert_eq!(carried, 3, "the snapshot it is new in and the next two");
    assert_eq!(seen_on_the_stream, 1, "once on the reliable lane");
}

#[tokio::test(start_paused = true)]
async fn events_are_never_lost_on_a_lossy_link_and_every_id_is_distinct() {
    let mut harness = Harness::new(Conditions {
        delay: Duration::from_millis(30),
        jitter: Duration::from_millis(20),
        loss: 0.5,
        seed: 5,
    });
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];

    // Through the first bot lap: the reliable lane carries every event,
    // and whichever snapshots arrive repeat only events already seen or
    // about to be seen there, under the same ids.
    let mut on_the_stream: Vec<Event> = Vec::new();
    let mut in_snapshots: BTreeSet<u32> = BTreeSet::new();
    while on_the_stream.len() < KARTS_PER_RACE - 1 {
        match next_events_or_snapshot(player).await.unwrap() {
            Ok(events) => on_the_stream.extend(events),
            Err(snapshot) => in_snapshots.extend(snapshot.events.iter().map(|event| event.id)),
        }
    }
    let ids: BTreeSet<u32> = on_the_stream.iter().map(|event| event.id).collect();
    assert_eq!(
        ids.len(),
        on_the_stream.len(),
        "no id repeats on the stream"
    );
    assert!(
        in_snapshots
            .iter()
            .all(|id| ids.contains(id) || *id > *ids.iter().max().unwrap()),
        "a snapshot repeats only events the stream carries"
    );
    assert!(
        on_the_stream.iter().all(|event| matches!(
            event.kind,
            EventKind::LapCompleted { kart, .. }
                | EventKind::ItemPicked { kart, .. }
                | EventKind::ItemFired { kart, .. }
                | EventKind::CoinPicked { kart, .. }
                | EventKind::Hit { kart, .. }
                | EventKind::CoinsLost { kart, .. } if usize::from(kart) >= 1
        )),
        "the idle human does nothing; the bots race, take coins and hit each other: {on_the_stream:?}"
    );
}

/// The laps a lone human drives before the results of a race over `laps`.
async fn laps_raced_to_the_results(laps: u32) -> usize {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room_of_laps(laps);
    let mut players = vec![harness.connect()];
    players[0].join(&code, "khiet").await.unwrap();
    let countdown = start_race(&mut players).await;
    assert_eq!(countdown[0].laps, laps);
    let raced = drive_to_results(&mut players, &[(0, 0.0)]).await;
    let finisher = raced[0]
        .results
        .iter()
        .find(|standing| standing.kart == 0)
        .expect("the human is placed");
    assert!(finisher.finish_time_s.is_some(), "the human finished");
    raced[0]
        .events
        .iter()
        .filter(|(_, event)| matches!(event.kind, EventKind::LapCompleted { kart: 0, .. }))
        .count()
}

#[tokio::test(start_paused = true)]
async fn a_race_of_three_laps_ends_after_three() {
    assert_eq!(laps_raced_to_the_results(3).await, 3);
}

#[tokio::test(start_paused = true)]
async fn a_race_of_five_laps_ends_after_five() {
    assert_eq!(laps_raced_to_the_results(5).await, 5);
}

#[tokio::test(start_paused = true)]
async fn a_room_of_two_gets_its_results_only_once_both_humans_have_finished_and_the_outro_has_run()
{
    let mut harness = Harness::new(Conditions::PERFECT);
    let (code, mut players, _) = racing_room(&mut harness, 2).await;

    // The second human sets off well behind the first.
    let raced = drive_to_results(&mut players, &[(0, 0.0), (1, 20.0)]).await;
    let heard = &raced[0];
    let first = heard.finish_tick(0).expect("the first human finished");
    let last = heard.finish_tick(1).expect("the second human finished");
    assert!(
        seconds(last - first) > 2.0 * RACE_OUTRO_S,
        "the humans finished {} s apart",
        seconds(last - first)
    );
    let ended = heard
        .events
        .iter()
        .find_map(|&(tick, event)| (event.kind == EventKind::RaceEnded).then_some(tick))
        .expect("the end of the race is told");
    assert_eq!(ended, last, "the race ends as the last human finishes");
    let after_the_last = seconds(heard.results_tick - last);
    assert!(
        (after_the_last - RACE_OUTRO_S).abs() <= 0.5,
        "the results came {after_the_last} s after the last human finished"
    );
    for other in &raced[1..] {
        assert_eq!(
            other.results, heard.results,
            "everyone gets the same results"
        );
    }

    let results = &heard.results;
    assert_eq!(results.len(), KARTS_PER_RACE);
    assert_eq!(
        results
            .iter()
            .map(|standing| standing.position)
            .collect::<Vec<_>>(),
        (1..=KARTS_PER_RACE as u8).collect::<Vec<_>>(),
        "in finishing order"
    );
    for human in [0, 1] {
        let standing = results
            .iter()
            .find(|standing| standing.kart == human)
            .unwrap();
        assert!(standing.finish_time_s.is_some() && standing.best_lap_s.is_some());
    }
    // Finishers first, then whoever was still racing, placed by progress.
    let finished = results
        .iter()
        .take_while(|standing| standing.finish_time_s.is_some())
        .count();
    assert!(results[finished..]
        .iter()
        .all(|standing| standing.finish_time_s.is_none()));

    // The room shows Results and both players are Finished.
    for player in &mut players {
        let room = player.next_room().await.unwrap();
        assert_eq!(room.state, RoomState::Results);
        assert!(room
            .players
            .iter()
            .all(|player| player.state == PlayerState::Finished));
    }
    assert_eq!(
        harness.rooms().view(&code).unwrap().state,
        RoomState::Results
    );
}

#[tokio::test(start_paused = true)]
async fn a_humans_inputs_drive_their_kart_through_the_first_gate() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];

    // The oval's front straight runs through the first gate; full throttle
    // from Go reaches it, and the snapshot reports the kart past it.
    let mut sequence = 0;
    loop {
        match player.recv().await.unwrap() {
            ServerMessage::Snapshot(snapshot) => {
                // Throttle before the start window would stall the kart.
                if seconds(snapshot.tick) >= COUNTDOWN_S {
                    sequence += 1;
                    player.send_input(input(sequence, u8::MAX));
                }
                if snapshot.karts[0].checkpoint >= 1 {
                    assert_eq!(snapshot.karts[0].lap, 1);
                    assert!(snapshot.acknowledged[0] > 0, "driven by the inputs sent");
                    break;
                }
                assert!(
                    seconds(snapshot.tick) < COUNTDOWN_S + 20.0,
                    "no checkpoint in 20 s: {:?}",
                    snapshot.karts[0]
                );
            }
            ServerMessage::Events(_) | ServerMessage::Room(_) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[tokio::test(start_paused = true)]
async fn rematch_restarts_with_the_same_room_and_players_and_a_newcomer_joins_between_races() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (code, mut players, first) = racing_room(&mut harness, 2).await;
    let raced = drive_to_results(&mut players, &[(0, 0.0), (1, 0.0)]).await;
    let highest_id = raced.iter().map(Raced::highest_id).max().unwrap();
    for player in players.iter_mut() {
        let room = player.next_room().await.unwrap();
        assert_eq!(room.state, RoomState::Results);
    }

    // Between races the room takes a newcomer.
    let mut newcomer = harness.connect();
    let (slot, room) = newcomer.join(&code, "new").await.unwrap();
    assert_eq!(slot, 2);
    assert_eq!(room.state, RoomState::Results);
    for player in players.iter_mut() {
        player.next_room().await.unwrap();
    }

    // The first player back from the results reopens the room; the other
    // is still Finished until they come back too.
    players[1].send(&ClientMessage::Rematch);
    let room = players[0].next_room().await.unwrap();
    assert_eq!(room.state, RoomState::Open);
    assert_eq!(room.players[0].state, PlayerState::Finished);
    assert_eq!(room.players[1].state, PlayerState::Unready);
    assert_eq!(room.players[2].state, PlayerState::Unready);
    players[1].next_room().await.unwrap();
    newcomer.next_room().await.unwrap();
    players[0].send(&ClientMessage::Rematch);
    let room = players[0].next_room().await.unwrap();
    assert_eq!(room.players[0].state, PlayerState::Unready);
    assert!(room.players[0].creator, "the creator keeps the role");
    players[1].next_room().await.unwrap();
    newcomer.next_room().await.unwrap();

    // The same room races again with the newcomer on the grid behind.
    players.push(newcomer);
    let second = start_race(&mut players).await;
    assert_eq!(second[0].track, first[0].track);
    assert_eq!(second[0].grid, vec![0, 1, 2]);
    assert_ne!(second[0].seed, first[0].seed, "a fresh seed");

    // Event ids carry on from the first race, so nothing de-duplicated
    // from it is mistaken for a repeat.
    let first_id_of_second_race = players[0].next_events().await.unwrap()[0].id;
    assert!(
        first_id_of_second_race > highest_id,
        "{first_id_of_second_race} continues past the first race's {highest_id}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_player_who_leaves_mid_race_frees_their_seat_at_the_results_and_the_rest_race_on() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 2).await;
    let leaver = players.remove(1);
    // The leaver rocket-starts, holding the throttle from the count a bot
    // holds from, and is driving when they go; a bot drives their kart
    // after.
    let mut snapshot = players[0].next_snapshot().await.unwrap();
    while seconds(snapshot.tick) < COUNTDOWN_S - BOT_START_HOLD_S {
        snapshot = players[0].next_snapshot().await.unwrap();
    }
    for sequence in 1..=INPUT_QUEUE_START_TICKS as u32 {
        leaver.send_input(input(sequence, u8::MAX));
    }
    while speed(&snapshot, 1) < 5.0 {
        snapshot = players[0].next_snapshot().await.unwrap();
    }
    leaver.send(&ClientMessage::Leave);
    let room = players[0].next_room().await.unwrap();
    assert_eq!(room.players.len(), 2);
    assert_eq!(room.players[1].state, PlayerState::Disconnected);

    let results = drive_to_results(&mut players, &[(0, 0.0)])
        .await
        .remove(0)
        .results;
    assert_eq!(results.len(), KARTS_PER_RACE);
    let left = results.iter().find(|standing| standing.kart == 1).unwrap();
    assert!(
        left.best_lap_s.is_some(),
        "the bot drove the left kart on round the laps: {left:?}"
    );
    let room = players[0].next_room().await.unwrap();
    assert_eq!(room.state, RoomState::Results);
    assert_eq!(room.players.len(), 1, "the seat freed at the results");
}

#[tokio::test(start_paused = true)]
async fn a_room_whose_finishers_all_leave_can_still_start_from_results() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (code, mut players, _) = racing_room(&mut harness, 1).await;
    drive_to_results(&mut players, &[(0, 0.0)]).await;
    players[0].next_room().await.unwrap();

    // The newcomer joins while the room shows Results; the finisher leaves
    // without a Rematch, so the newcomer is the creator of a Results room.
    let mut newcomer = harness.connect();
    newcomer.join(&code, "new").await.unwrap();
    players[0].next_room().await.unwrap();
    players.remove(0).send(&ClientMessage::Leave);
    let room = newcomer.next_room().await.unwrap();
    assert_eq!(room.state, RoomState::Results);
    assert_eq!(room.players.len(), 1);
    assert!(room.players[0].creator);

    newcomer.send(&ClientMessage::Ready { ready: true });
    newcomer.next_room().await.unwrap();
    newcomer.send(&ClientMessage::Start);
    let countdown = newcomer.next_countdown().await.unwrap();
    assert_eq!(countdown.grid, vec![1]);
    assert_eq!(
        newcomer.next_room().await.unwrap().state,
        RoomState::Countdown
    );
}

#[tokio::test(start_paused = true)]
async fn stopping_the_server_mid_race_ends_every_session() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 2).await;
    players[0].next_snapshot().await.unwrap();
    harness.stop();
    for player in &mut players {
        assert_eq!(
            player
                .next_where(|message| match message {
                    ServerMessage::Results { .. } => Some(()),
                    _ => None,
                })
                .await,
            None,
            "no results come: the session ended"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn an_item_pickup_and_its_fire_are_server_events_the_snapshots_repeat_and_show() {
    use sim::tuning::{BOT_ITEM_USE_DELAY_S, ITEM_ROLL_S};
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut players, _) = racing_room(&mut harness, 1).await;
    let player = &mut players[0];

    // The idle human watches the bots; the first through the oval's boxes
    // picks one up, which the reliable lane reports with the box.
    let mut last_tick = 0;
    let picked = loop {
        match next_events_or_snapshot(player).await.unwrap() {
            Ok(events) => {
                if let Some(event) = events
                    .iter()
                    .find(|event| matches!(event.kind, EventKind::ItemPicked { .. }))
                {
                    break *event;
                }
            }
            Err(snapshot) => {
                assert!(
                    snapshot
                        .karts
                        .iter()
                        .all(|kart| kart.item_slot == ItemSlot::Empty),
                    "no item before the first pickup is reported"
                );
                last_tick = snapshot.tick;
            }
        }
    };
    let EventKind::ItemPicked {
        kart,
        item: Some(item),
        item_box,
    } = picked.kind
    else {
        panic!("the first pickup is into an empty slot: {picked:?}")
    };
    assert!(kart >= 1, "a bot took it: {picked:?}");
    let boxes = sim::Track::from_sidecar(OVAL_SIDECAR)
        .expect("the oval loads")
        .item_boxes()
        .len();
    assert!(usize::from(item_box) < boxes, "one of the oval's boxes");
    let picked_tick = last_tick;

    // The snapshots repeat the pickup under its id and show the item
    // rolling in the kart's slot, then landed, until the bot fires it
    // within the tuned delay of the landing; the fire is reported the same
    // way and empties the slot.
    let mut pickup_repeats = 0;
    let mut fired = None;
    let mut fire_repeats = 0;
    let mut after_fire = 0;
    while after_fire < 3 {
        match next_events_or_snapshot(player).await.unwrap() {
            Ok(events) => {
                assert!(
                    events.iter().all(|event| event.id != picked.id),
                    "once on the reliable lane"
                );
                if let Some(event) = events.iter().find(|event| {
                    matches!(event.kind, EventKind::ItemFired { kart: by, .. } if by == kart)
                }) {
                    assert_eq!(fired, None, "fired once");
                    assert_eq!(event.kind, EventKind::ItemFired { kart, item });
                    assert!(event.id > picked.id);
                    fired = Some(*event);
                    let after_s = seconds(last_tick - picked_tick);
                    assert!(
                        (ITEM_ROLL_S - 0.1..=ITEM_ROLL_S + BOT_ITEM_USE_DELAY_S + 0.1)
                            .contains(&after_s),
                        "fired {after_s} s after the pickup"
                    );
                }
            }
            Err(snapshot) => {
                last_tick = snapshot.tick;
                let slot = snapshot.karts[usize::from(kart)].item_slot;
                pickup_repeats += snapshot
                    .events
                    .iter()
                    .filter(|event| **event == picked)
                    .count();
                match fired {
                    None => assert!(
                        matches!(
                            slot,
                            ItemSlot::Rolling { item: rolled, .. } | ItemSlot::Held(rolled)
                                if rolled == item
                        ),
                        "rolling, then held, until fired: {slot:?}"
                    ),
                    Some(fired) => {
                        fire_repeats += snapshot
                            .events
                            .iter()
                            .filter(|event| **event == fired)
                            .count();
                        after_fire += 1;
                    }
                }
            }
        }
    }
    assert_eq!(pickup_repeats, 3, "the pickup rode three snapshots");
    assert_eq!(fire_repeats, 3, "the fire rode three snapshots");
}
