//! Open rooms survive a server restart (ADR 0005, ADR 0012), driven through
//! the fake transport: a server stopped and another started over the same
//! directory brings back every room that was Open, with its seats and
//! their session tokens, so the players rejoin where they were. A room in
//! a race is lost, a seat nobody reclaims frees after the tuned grace, and
//! a room that closed is not brought back.

mod common;

use std::time::Duration;

use common::race_together;
use protocol::{ClientMessage, PlayerState, Refusal, Room, RoomState};
use server::fake_transport::{Client, Conditions, Harness};
use sim::tuning::{RESTORED_SEAT_GRACE_S, ROOM_CLOSE_AFTER_EMPTY_S};

/// Two players in an Open room, both Ready, as a room stands while its
/// creator reaches for Start. Returns the code and the clients in slot
/// order, each having heard the room with both Ready.
async fn two_ready(harness: &mut Harness) -> (String, Vec<Client>) {
    let code = harness.create_room();
    let mut clients = Vec::new();
    for index in 0..2 {
        let mut client = harness.connect();
        client.join(&code, &format!("player{index}")).await.unwrap();
        clients.push(client);
    }
    for client in &clients {
        client.send(&ClientMessage::Ready { ready: true });
    }
    for client in clients.iter_mut() {
        room_where(client, |room| {
            room.players.len() == 2
                && room
                    .players
                    .iter()
                    .all(|player| player.state == PlayerState::Ready)
        })
        .await;
    }
    (code, clients)
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

#[tokio::test(start_paused = true)]
async fn an_open_room_with_two_ready_players_is_back_after_a_restart_and_both_rejoin_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::kept_in(Conditions::PERFECT, dir.path());
    let (code, clients) = two_ready(&mut before).await;
    let tokens: Vec<String> = clients
        .iter()
        .map(|client| client.token().unwrap().to_owned())
        .collect();

    // A deploy: the server stops with the room standing, and a new one
    // starts over the same directory.
    before.stop();
    drop(clients);
    drop(before);
    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    assert_eq!(after.rooms().codes(), vec![code.clone()]);

    // Each client's reconnect: a Rejoin with the token it kept.
    let mut back = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let mut client = after.connect();
        let (slot, room) = client.rejoin(&code, token).await.unwrap();
        assert_eq!(usize::from(slot), index, "the same seat");
        assert_eq!(room.code, code);
        assert_eq!(room.state, RoomState::Open, "still in the Room Scene");
        assert_eq!(room.players.len(), 2, "both seats kept");
        assert!(
            room.players
                .iter()
                .all(|player| player.state == PlayerState::Ready),
            "ready as they were: {:?}",
            room.players
        );
        assert_eq!(room.players[0].nickname, "player0");
        assert!(room.players[0].creator, "the creator is still the creator");
        back.push(client);
    }

    // The room goes on: the creator starts it and both are told.
    back[0].send(&ClientMessage::Start);
    for client in back.iter_mut() {
        let started = client.next_countdown().await.expect("the race starts");
        assert_eq!(started.grid, vec![0, 1]);
    }
}

#[tokio::test(start_paused = true)]
async fn a_room_in_a_race_at_the_restart_is_lost() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::kept_in(Conditions::PERFECT, dir.path());
    let code = before.create_room();
    let mut players = vec![before.connect(), before.connect()];
    race_together(&code, &mut players).await;
    let token = players[0].token().unwrap().to_owned();

    before.stop();
    drop(players);
    drop(before);
    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    assert!(after.rooms().codes().is_empty(), "no room comes back");
    let mut client = after.connect();
    assert_eq!(
        client.rejoin(&code, &token).await,
        Err(Refusal::UnknownRoom)
    );
}

#[tokio::test(start_paused = true)]
async fn a_restored_seat_nobody_reclaims_frees_after_the_grace() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::kept_in(Conditions::PERFECT, dir.path());
    let (code, clients) = two_ready(&mut before).await;
    let creator_token = clients[0].token().unwrap().to_owned();
    let friend_token = clients[1].token().unwrap().to_owned();
    before.stop();
    drop(clients);
    drop(before);

    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    let mut friend = after.connect();
    let (slot, room) = friend.rejoin(&code, &friend_token).await.unwrap();
    assert_eq!(slot, 1);
    assert_eq!(room.players.len(), 2, "the creator's seat waits");
    assert!(!room.players[1].creator);

    // The creator never comes back: their seat frees and the role passes.
    let room = room_where(&mut friend, |room| room.players.len() == 1).await;
    assert_eq!(room.players[0].slot, 1);
    assert!(room.players[0].creator, "the creator role passed");
    assert_eq!(room.state, RoomState::Open);
    // The seat is gone for good: its token is refused.
    let mut late = after.connect();
    assert_eq!(
        late.rejoin(&code, &creator_token).await,
        Err(Refusal::UnknownSession)
    );
    // Alone and Ready, the friend starts the next race.
    friend.send(&ClientMessage::Start);
    friend.next_countdown().await.expect("the race starts");
}

#[tokio::test(start_paused = true)]
async fn a_restored_room_nobody_returns_to_closes_and_stays_gone() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::kept_in(Conditions::PERFECT, dir.path());
    let (code, clients) = two_ready(&mut before).await;
    before.stop();
    drop(clients);
    drop(before);

    let after = Harness::kept_in(Conditions::PERFECT, dir.path());
    let mut watching = after.rooms().watch(&code).expect("the room is back");
    tokio::time::sleep(Duration::from_secs_f32(
        RESTORED_SEAT_GRACE_S + ROOM_CLOSE_AFTER_EMPTY_S + 1.0,
    ))
    .await;
    watching.changed().await.ok();
    assert_eq!(watching.borrow().state, RoomState::Closing);
    tokio::task::yield_now().await;
    assert!(after.rooms().codes().is_empty());

    after.stop();
    drop(after);
    let again = Harness::kept_in(Conditions::PERFECT, dir.path());
    assert!(
        again.rooms().codes().is_empty(),
        "a closed room is not kept"
    );
}
