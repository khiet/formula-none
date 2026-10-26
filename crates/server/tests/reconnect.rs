//! A connection lost mid-race (ADR 0012), driven through the fake
//! transport: a bot drives the kart of a human who drops or falls silent,
//! the session token the join dealt takes the seat back, any other token
//! is refused, and a human who never comes back is raced to the results by
//! the bot and their seat freed there.

mod common;

use common::{drive_to_results, race_together, seconds, speed};
use protocol::{Input, PlayerState, Refusal, Room, RoomState, ServerMessage, Snapshot};
use server::fake_transport::{Client, Conditions, Harness};
use sim::tuning::{
    CONNECTION_SILENCE_S, COUNTDOWN_S, INPUT_QUEUE_MAX_TICKS, KARTS_PER_RACE, TICKS_PER_SECOND,
};

fn input(sequence: u32, throttle: u8, brake: u8) -> Input {
    Input {
        sequence,
        steer: 0,
        throttle,
        brake,
        hop: false,
        use_item: false,
        hold_item: false,
    }
}

/// The next room update `pick` accepts, skipping the rest.
async fn room_where(client: &mut Client, pick: impl Fn(&Room) -> bool) -> Room {
    loop {
        let room = client.next_room().await.expect("the room goes on");
        if pick(&room) {
            return room;
        }
    }
}

/// The first snapshot at or past `at_s` on the race clock.
async fn snapshot_at(client: &mut Client, at_s: f32) -> Snapshot {
    loop {
        let snapshot = client.next_snapshot().await.expect("the race goes on");
        if seconds(snapshot.tick) >= at_s {
            return snapshot;
        }
    }
}

/// Two humans racing the oval: the first is the one whose connection the
/// test plays with, the second watches.
async fn two_racing(harness: &mut Harness) -> (String, Client, Client) {
    let code = harness.create_room();
    let mut players = vec![harness.connect(), harness.connect()];
    race_together(&code, &mut players).await;
    let watcher = players.pop().unwrap();
    let player = players.pop().unwrap();
    (code, player, watcher)
}

#[tokio::test(start_paused = true)]
async fn a_bot_drives_a_dropped_humans_kart_until_their_token_rejoins_and_hands_it_back() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (code, player, mut watcher) = two_racing(&mut harness).await;
    let token = player.token().expect("the join dealt a token").to_owned();

    player.disconnect();
    let room = room_where(&mut watcher, |room| {
        room.players[0].state == PlayerState::Disconnected
    })
    .await;
    assert_eq!(room.players.len(), 2, "the seat is kept");
    // Nobody sends kart 0 an input, yet it races.
    let snapshot = snapshot_at(&mut watcher, COUNTDOWN_S + 4.0).await;
    assert!(
        speed(&snapshot, 0) > 15.0,
        "the bot drives the kart: {:?}",
        snapshot.karts[0]
    );

    let mut back = harness.connect();
    let (slot, room) = back.rejoin(&code, &token).await.unwrap();
    assert_eq!(slot, 0, "the same seat");
    assert_eq!(room.players[0].state, PlayerState::Racing);
    assert_eq!(
        back.token(),
        Some(token.as_str()),
        "a rejoin keeps the token"
    );
    let countdown = back.next_countdown().await.unwrap();
    assert_eq!(
        countdown.grid,
        vec![0, 1],
        "the race under way, to rebuild from"
    );
    room_where(&mut watcher, |room| {
        room.players[0].state == PlayerState::Racing
    })
    .await;

    // The rejoined client brakes from its own first sequence: the server
    // applies its inputs and the kart stops under them.
    let mut sequence = 0;
    let stopped = loop {
        let snapshot = back.next_snapshot().await.unwrap();
        sequence += 1;
        back.send_input(input(sequence, 0, u8::MAX));
        if snapshot.acknowledged[0] > 0 && speed(&snapshot, 0) < 0.5 {
            break snapshot;
        }
        assert!(
            sequence < 10 * TICKS_PER_SECOND,
            "the kart never stopped: {:?}",
            snapshot.karts[0]
        );
    };
    // The acknowledgement is the sequence applied, which runs on a tick
    // ahead of a client sending one input per snapshot, up to the queue's
    // depth (ADR 0019): never further than that past what was sent.
    assert!(
        stopped.acknowledged[0] <= sequence + INPUT_QUEUE_MAX_TICKS,
        "acknowledged {} with {sequence} sent",
        stopped.acknowledged[0]
    );
}

#[tokio::test(start_paused = true)]
async fn a_rejoin_with_any_other_token_is_refused_and_the_seat_waits_for_its_own() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (code, player, mut watcher) = two_racing(&mut harness).await;
    let token = player.token().unwrap().to_owned();
    let other_token = watcher.token().unwrap().to_owned();
    player.disconnect();
    room_where(&mut watcher, |room| {
        room.players[0].state == PlayerState::Disconnected
    })
    .await;

    let mut stranger = harness.connect();
    assert_eq!(
        stranger
            .rejoin(&code, "00000000000000000000000000000000")
            .await,
        Err(Refusal::UnknownSession)
    );
    assert_eq!(stranger.recv().await, None, "a refusal ends the session");
    // The watcher's own token names the watcher's seat, not the empty one.
    let mut impostor = harness.connect();
    let (slot, _) = impostor.rejoin(&code, &other_token).await.unwrap();
    assert_eq!(slot, 1);

    let mut back = harness.connect();
    let (slot, _) = back.rejoin(&code, &token).await.unwrap();
    assert_eq!(slot, 0);
}

#[tokio::test(start_paused = true)]
async fn a_rejoin_while_the_old_connection_lives_replaces_it() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (code, mut player, mut watcher) = two_racing(&mut harness).await;
    let token = player.token().unwrap().to_owned();

    let mut second = harness.connect();
    let (slot, _) = second.rejoin(&code, &token).await.unwrap();
    assert_eq!(slot, 0);
    assert_eq!(
        player
            .next_where(|message| match message {
                ServerMessage::Replaced => Some(()),
                _ => None,
            })
            .await,
        Some(()),
        "the old connection is told"
    );

    // The old connection going now is not the player going, who races on
    // over the new one.
    player.disconnect();
    let mut sequence = 0;
    loop {
        let snapshot = second.next_snapshot().await.unwrap();
        sequence += 1;
        second.send_input(input(sequence, 0, 0));
        if seconds(snapshot.tick) >= COUNTDOWN_S + CONNECTION_SILENCE_S + 1.0 {
            break;
        }
    }
    assert_eq!(
        harness.rooms().view(&code).unwrap().players[0].state,
        PlayerState::Racing
    );
    assert!(watcher.next_snapshot().await.is_some());
}

#[tokio::test(start_paused = true)]
async fn a_human_silent_for_the_tuned_time_is_handed_to_a_bot_and_their_next_input_takes_it_back() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut player, mut watcher) = two_racing(&mut harness).await;

    // The player drives from Go, then goes quiet on a connection that
    // stays up: a blip on the way to the server.
    let mut sequence = 0;
    loop {
        let snapshot = player.next_snapshot().await.unwrap();
        if seconds(snapshot.tick) >= COUNTDOWN_S {
            sequence += 1;
            player.send_input(input(sequence, u8::MAX, 0));
        }
        if seconds(snapshot.tick) >= COUNTDOWN_S + 1.0 {
            break;
        }
    }
    let went_quiet = snapshot_at(&mut watcher, COUNTDOWN_S + 1.0).await.tick;
    let room = room_where(&mut watcher, |room| {
        room.players[0].state == PlayerState::Disconnected
    })
    .await;
    assert_eq!(room.state, RoomState::Racing);
    assert_eq!(
        room.players[1].state,
        PlayerState::Racing,
        "a client yet to send its first input is not silent"
    );
    let handed_at = watcher.next_snapshot().await.unwrap().tick;
    let silent_s = seconds(handed_at - went_quiet);
    assert!(
        (silent_s - CONNECTION_SILENCE_S).abs() <= 0.25,
        "handed to a bot after {silent_s} s"
    );

    // The connection never dropped, so the next input is the player back.
    sequence += 1;
    player.send_input(input(sequence, u8::MAX, 0));
    room_where(&mut watcher, |room| {
        room.players[0].state == PlayerState::Racing
    })
    .await;
}

#[tokio::test(start_paused = true)]
async fn a_human_who_never_comes_back_is_raced_by_the_bot_until_the_rest_finish_and_their_seat_freed(
) {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (code, player, mut watcher) = two_racing(&mut harness).await;
    let token = player.token().unwrap().to_owned();
    player.disconnect();

    // The dropped kart does not hold the race open; the watcher's finish ends it.
    let standings = drive_to_results(std::slice::from_mut(&mut watcher), &[(1, 0.0)])
        .await
        .remove(0)
        .results;
    assert_eq!(standings.len(), KARTS_PER_RACE);
    let dropped = standings
        .iter()
        .find(|standing| standing.kart == 0)
        .unwrap();
    assert!(
        dropped.best_lap_s.is_some(),
        "the bot raced the dropped human's kart round the laps: {dropped:?}"
    );
    let room = room_where(&mut watcher, |room| room.state == RoomState::Results).await;
    assert_eq!(room.players.len(), 1, "the dropped human's seat freed");
    assert_eq!(room.players[0].state, PlayerState::Finished);

    let mut late = harness.connect();
    assert_eq!(
        late.rejoin(&code, &token).await,
        Err(Refusal::UnknownSession),
        "the race the seat was kept for is over"
    );
}

#[tokio::test(start_paused = true)]
async fn a_rejoin_between_races_is_refused_while_the_seat_is_held_so_a_second_tab_joins_afresh() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut player = harness.connect();
    player.join(&code, "ada").await.unwrap();
    let token = player.token().unwrap().to_owned();

    let mut second_tab = harness.connect();
    assert_eq!(
        second_tab.rejoin(&code, &token).await,
        Err(Refusal::SessionInUse)
    );
    let mut second_tab = harness.connect();
    let (slot, room) = second_tab.join(&code, "ada").await.unwrap();
    assert_eq!(slot, 1, "a new player beside the first");
    assert_eq!(room.players.len(), 2);
    assert_eq!(
        player.next_room().await.unwrap().players.len(),
        2,
        "the first tab keeps its seat and hears of the second"
    );
}

#[tokio::test(start_paused = true)]
async fn a_player_silent_but_still_connected_at_the_finish_keeps_their_seat_and_gets_the_results() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let (_, mut player, mut watcher) = two_racing(&mut harness).await;
    // A few inputs, then nothing: a tab in the background through the end.
    for sequence in 1..=3 {
        player.send_input(input(sequence, 0, 0));
    }
    room_where(&mut watcher, |room| {
        room.players[0].state == PlayerState::Disconnected
    })
    .await;

    // The silent kart is the bot's and holds nothing open; the watcher's
    // finish ends the race.
    drive_to_results(std::slice::from_mut(&mut watcher), &[(1, 0.0)]).await;
    assert!(
        player.next_results().await.is_some(),
        "still told the results"
    );
    let room = room_where(&mut watcher, |room| room.state == RoomState::Results).await;
    assert_eq!(room.players.len(), 2, "the seat is kept");
    assert!(room
        .players
        .iter()
        .all(|player| player.state == PlayerState::Finished));
}
