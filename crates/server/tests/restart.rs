//! Open rooms survive a server restart (ADR 0005, ADR 0012), driven through
//! the fake transport: a server stopped and another started over the same
//! directory brings back every room that was Open, with its seats and
//! their session tokens, so the players rejoin where they were. A room in
//! a race is lost, a seat nobody reclaims frees after the tuned grace, and
//! a room that closed is not brought back.

mod common;

use std::time::Duration;

use common::{drive_to_results, race_together};
use protocol::{ClientMessage, PlayerState, Refusal, Room, RoomState, ServerMessage};
use server::fake_transport::{Client, Conditions, Harness};
use sim::tuning::{RESTORED_SEAT_GRACE_S, ROOM_CLOSE_AFTER_EMPTY_S};
use sim::{BotLevel, Picks, Tyres};

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
        // Each is Ready again as they come back; a seat still waiting for
        // its player shows Disconnected.
        let states: Vec<PlayerState> = room.players.iter().map(|player| player.state).collect();
        let expected = (0..2)
            .map(|seat| {
                if seat <= index {
                    PlayerState::Ready
                } else {
                    PlayerState::Disconnected
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(states, expected, "as they were once back");
        assert_eq!(room.players[0].nickname, "player0");
        assert!(room.players[0].creator, "the creator is still the creator");
        back.push(client);
    }
    // The first back is told the second is.
    let room = room_where(&mut back[0], |room| {
        room.players
            .iter()
            .all(|player| player.state == PlayerState::Ready)
    })
    .await;
    assert_eq!(room.players.len(), 2);

    // The room goes on: the creator starts it and both are told.
    back[0].send(&ClientMessage::Start);
    for client in back.iter_mut() {
        let started = client.next_countdown().await.expect("the race starts");
        assert_eq!(started.grid, vec![0, 1]);
    }
}

#[tokio::test(start_paused = true)]
async fn a_room_nobody_has_joined_yet_is_back_too() {
    let dir = tempfile::tempdir().unwrap();
    let before = Harness::kept_in(Conditions::PERFECT, dir.path());
    // The creator was given the code and is about to join it.
    let code = before.create_room();
    before.stop();
    drop(before);

    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    let mut creator = after.connect();
    let (slot, room) = creator.join(&code, "khiet").await.unwrap();
    assert_eq!(slot, 0);
    assert!(room.players[0].creator);
}

/// The file the last deploy wrote, as `KeptRoom` in `crates/server/src/room.rs`
/// serializes it and `crates/server/src/kept.rs` names it: a deploy must read
/// what the one before left, so the spelling is a contract and a change to
/// it needs a migration.
const KEPT_FILE: &str = r#"{"code":"WXYZ","track":"oval","state":"open","seats":[{"player":{"slot":0,"nickname":"khiet","livery":0,"state":"ready","creator":true},"joined_at":1,"token":"0123456789abcdef0123456789abcdef"},null,null,null,null,null,null,null],"joins":1,"next_event_id":1}"#;

#[tokio::test(start_paused = true)]
async fn a_file_the_last_deploy_wrote_reads_back_and_its_token_rejoins() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("WXYZ.json"), KEPT_FILE).unwrap();

    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    let mut client = after.connect();
    let (slot, room) = client
        .rejoin("WXYZ", "0123456789abcdef0123456789abcdef")
        .await
        .unwrap();
    assert_eq!(slot, 0);
    assert_eq!(room.state, RoomState::Open);
    assert_eq!(room.players[0].nickname, "khiet");
    assert_eq!(room.players[0].state, PlayerState::Ready);
}

/// The track a room kept on `track` races once a restart brings it back
/// and its creator rejoins and presses Start, with the server that ran it.
async fn track_raced_after_keeping_a_room_on(track: &str) -> (String, Harness) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("WXYZ.json"),
        KEPT_FILE.replace("\"track\":\"oval\"", &format!("\"track\":\"{track}\"")),
    )
    .unwrap();

    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    assert_eq!(after.rooms().codes(), vec!["WXYZ".to_owned()]);
    let mut creator = after.connect();
    creator
        .rejoin("WXYZ", "0123456789abcdef0123456789abcdef")
        .await
        .unwrap();
    creator.send(&ClientMessage::Start);
    let started = creator.next_countdown().await.expect("the race starts");
    (started.track, after)
}

#[tokio::test(start_paused = true)]
async fn a_kept_room_on_brands_patch_races_bell_deer_its_new_name() {
    let (track, after) = track_raced_after_keeping_a_room_on("brands-patch").await;
    assert_eq!(track, "bell-deer");
    assert!(
        !after
            .logs()
            .iter()
            .any(|line| line["event"] == "room_track_unknown"),
        "a renamed track is no surprise"
    );
}

#[tokio::test(start_paused = true)]
async fn a_kept_room_on_a_track_this_server_lacks_races_bell_deer_with_a_warning() {
    let (track, after) = track_raced_after_keeping_a_room_on("monza").await;
    assert_eq!(track, "bell-deer");
    assert!(after
        .logs()
        .iter()
        .any(|line| line["event"] == "room_track_unknown"
            && line["room"] == "WXYZ"
            && line["track"] == "monza"));
}

#[tokio::test(start_paused = true)]
async fn a_stray_file_is_reported_and_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.json"), "not a room").unwrap();

    let after = Harness::kept_in(Conditions::PERFECT, dir.path());
    assert!(after.rooms().codes().is_empty());
    assert!(
        dir.path().join("notes.json").exists(),
        "a stray file is not ours to remove"
    );
    assert!(after
        .logs()
        .iter()
        .any(|line| line["event"] == "room_unreadable"));
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
    assert_eq!(room.players[1].state, PlayerState::Ready, "as they were");
    assert!(!room.players[1].creator);
    assert_eq!(
        room.players[0].state,
        PlayerState::Disconnected,
        "the seat waits Disconnected, so nobody starts around them"
    );

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
async fn a_waiting_seat_blocks_start_until_it_is_reclaimed() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::kept_in(Conditions::PERFECT, dir.path());
    let (code, clients) = two_ready(&mut before).await;
    let creator_token = clients[0].token().unwrap().to_owned();
    let friend_token = clients[1].token().unwrap().to_owned();
    before.stop();
    drop(clients);
    drop(before);

    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    let mut creator = after.connect();
    creator.rejoin(&code, &creator_token).await.unwrap();
    // The creator is back and Ready, the friend not yet: Start is ignored.
    creator.send(&ClientMessage::Start);
    let mut friend = after.connect();
    friend.rejoin(&code, &friend_token).await.unwrap();
    let heard = creator
        .next_where(|message| match message {
            ServerMessage::Countdown { .. } => Some("countdown"),
            ServerMessage::Room(room)
                if room
                    .players
                    .iter()
                    .all(|player| player.state == PlayerState::Ready) =>
            {
                Some("everyone back")
            }
            _ => None,
        })
        .await;
    assert_eq!(
        heard,
        Some("everyone back"),
        "no race started around the friend"
    );
    // Both back: Start is honoured.
    creator.send(&ClientMessage::Start);
    assert_eq!(creator.next_countdown().await.unwrap().grid, vec![0, 1]);
}

#[tokio::test(start_paused = true)]
async fn a_rematch_after_a_restart_continues_the_rooms_event_ids() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::kept_in(Conditions::PERFECT, dir.path());
    let code = before.create_room();
    let mut players = vec![before.connect()];
    race_together(&code, &mut players).await;
    let raced = drive_to_results(&mut players, &[(0, 0.0)]).await;
    let highest = raced[0].highest_id();
    assert!(highest > 0, "the first race had events");
    players[0].send(&ClientMessage::Rematch);
    room_where(&mut players[0], |room| room.state == RoomState::Open).await;
    let token = players[0].token().unwrap().to_owned();
    before.stop();
    drop(players);
    drop(before);

    // The client kept its highest id across the restart and drops any id
    // at or below it (ADR 0011), so the next race must count on from it.
    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    let mut back = after.connect();
    back.rejoin(&code, &token).await.unwrap();
    back.send(&ClientMessage::Ready { ready: true });
    room_where(&mut back, |room| {
        room.players[0].state == PlayerState::Ready
    })
    .await;
    back.send(&ClientMessage::Start);
    back.next_countdown().await.unwrap();
    let second = drive_to_results(std::slice::from_mut(&mut back), &[(0, 0.0)]).await;
    let lowest = second[0]
        .events
        .iter()
        .map(|(_, event)| event.id)
        .min()
        .expect("the second race had events");
    assert!(
        lowest > highest,
        "the second race's ids start past the first's: {lowest} after {highest}"
    );
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

/// The room `kept` describes, as a player rejoining it after a restart sees it.
async fn rejoined_after_a_restart(kept: &str) -> protocol::Room {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("WXYZ.json"), kept).unwrap();

    let mut after = Harness::kept_in(Conditions::PERFECT, dir.path());
    let mut client = after.connect();
    let (_, room) = client
        .rejoin("WXYZ", "0123456789abcdef0123456789abcdef")
        .await
        .unwrap();
    room
}

#[tokio::test(start_paused = true)]
async fn a_room_kept_before_the_picks_comes_back_as_before_there_were_any() {
    let room = rejoined_after_a_restart(KEPT_FILE).await;
    assert_eq!(room.picks, Picks::default());
}

#[tokio::test(start_paused = true)]
async fn a_room_kept_with_picks_comes_back_with_them() {
    let room = rejoined_after_a_restart(&KEPT_FILE.replace(
        "\"track\":\"oval\"",
        "\"track\":\"oval\",\"level\":\"ace\",\"tyres\":\"on\",\"laps\":5",
    ))
    .await;
    assert_eq!(
        room.picks,
        Picks {
            level: BotLevel::Ace,
            tyres: Tyres::On,
            laps: 5,
        }
    );
}

#[tokio::test(start_paused = true)]
async fn a_room_kept_before_the_picks_were_one_value_keeps_its_bot_level() {
    let room = rejoined_after_a_restart(&KEPT_FILE.replace(
        "\"track\":\"oval\"",
        "\"track\":\"oval\",\"bot_level\":\"ace\",\"tyres\":\"on\"",
    ))
    .await;
    assert_eq!(
        room.picks,
        Picks {
            level: BotLevel::Ace,
            tyres: Tyres::On,
            ..Picks::default()
        }
    );
}
