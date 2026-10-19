//! The room and player state machines (ADR 0012), driven through the fake
//! transport: a server room and its clients joined by channels, under
//! tokio's paused clock so the five-minute closing wait costs nothing.

use std::time::Duration;

use protocol::{ClientMessage, PlayerState, Refusal, RoomState, ServerMessage, PROTOCOL_VERSION};
use server::fake_transport::{Client, Conditions, Harness};
use sim::tuning::{
    NICKNAME_MAX_CHARS, ROOM_CLOSE_AFTER_EMPTY_S, ROOM_CODE_ALPHABET, ROOM_CODE_LENGTH,
    ROOM_PLAYERS_MAX,
};

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

#[tokio::test]
async fn a_created_room_is_joined_by_its_code_and_the_first_in_is_the_creator() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    assert_eq!(code.len(), ROOM_CODE_LENGTH);

    let mut creator = harness.connect();
    let (slot, room) = creator.join(&code, "khiet").await.unwrap();
    assert_eq!(slot, 0);
    assert_eq!(room.code, code);
    assert_eq!(room.state, RoomState::Open);
    assert_eq!(room.players.len(), 1);
    let me = &room.players[0];
    assert_eq!(me.nickname, "khiet");
    assert_eq!((me.slot, me.livery), (0, 0));
    assert_eq!(me.state, PlayerState::Unready);
    assert!(me.creator);

    let mut friend = harness.connect();
    let (slot, room) = friend.join(&code, "ada").await.unwrap();
    assert_eq!(slot, 1);
    assert_eq!(room.players.len(), 2);
    assert!(!room.players[1].creator);
    assert_eq!(room.players[1].livery, 1, "liveries follow join order");

    // The creator sees the friend arrive, in the same room they see.
    let update = creator.next_room().await.unwrap();
    assert_eq!(update, room);
}

#[tokio::test]
async fn the_code_is_read_as_typed_and_an_unknown_one_is_refused_and_ends_the_session() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();

    let mut typed_lowercase = harness.connect();
    let (_, room) = typed_lowercase
        .join(&format!(" {} ", code.to_ascii_lowercase()), "khiet")
        .await
        .unwrap();
    assert_eq!(room.code, code);

    for wrong in ["", "ZZZZ", "ABCDE", "O0IL", &code[..3]] {
        let mut client = harness.connect();
        assert_eq!(
            client.join(wrong, "ada").await,
            Err(Refusal::UnknownRoom),
            "{wrong:?}"
        );
        assert_eq!(
            client.recv().await,
            None,
            "the session ends after a refusal"
        );
    }
}

#[tokio::test]
async fn a_nickname_is_trimmed_and_cut_to_the_tuned_length() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut client = harness.connect();
    let (_, room) = client
        .join(&code, &format!("  {} ", "x".repeat(NICKNAME_MAX_CHARS + 5)))
        .await
        .unwrap();
    assert_eq!(room.players[0].nickname, "x".repeat(NICKNAME_MAX_CHARS));
}

#[tokio::test]
async fn a_ninth_human_is_refused_as_full() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let _eight = room_of(&mut harness, &code, ROOM_PLAYERS_MAX).await;

    let mut ninth = harness.connect();
    assert_eq!(ninth.join(&code, "ninth").await, Err(Refusal::RoomFull));
}

#[tokio::test]
async fn a_client_on_another_protocol_version_is_told_the_servers_so_it_reloads() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut stale = harness.connect();
    assert_eq!(
        stale
            .join_as_version(&code, "khiet", PROTOCOL_VERSION + 1)
            .await,
        Err(Refusal::VersionMismatch {
            server: PROTOCOL_VERSION
        })
    );
    // Before the room is even looked up, so a stale client never sees "unknown room".
    let mut stale_and_wrong = harness.connect();
    assert_eq!(
        stale_and_wrong
            .join_as_version("ZZZZ", "khiet", PROTOCOL_VERSION + 1)
            .await,
        Err(Refusal::VersionMismatch {
            server: PROTOCOL_VERSION
        })
    );
}

/// The next Room update as each client sees it; the room sends every
/// player the same one.
async fn same_update(clients: &mut [Client]) -> protocol::Room {
    let mut rooms = Vec::new();
    for client in clients.iter_mut() {
        rooms.push(client.next_room().await.unwrap());
    }
    assert!(rooms.windows(2).all(|pair| pair[0] == pair[1]));
    rooms.remove(0)
}

#[tokio::test]
async fn start_is_honoured_only_from_the_creator_once_every_human_is_ready() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = room_of(&mut harness, &code, 2).await;

    // Nobody ready: Start changes nothing, and the next change seen is the ready.
    players[0].send(&ClientMessage::Start);
    players[1].send(&ClientMessage::Ready { ready: true });
    let room = same_update(&mut players).await;
    assert_eq!(room.players[1].state, PlayerState::Ready);
    assert_eq!(room.state, RoomState::Open);

    // One ready: still nothing, and a non-creator's Start never counts.
    players[0].send(&ClientMessage::Start);
    players[1].send(&ClientMessage::Start);
    players[0].send(&ClientMessage::Ready { ready: true });
    let room = same_update(&mut players).await;
    assert_eq!(room.players[0].state, PlayerState::Ready);
    assert_eq!(room.state, RoomState::Open);
    players[1].send(&ClientMessage::Start);
    players[1].send(&ClientMessage::Ready { ready: false });
    let room = same_update(&mut players).await;
    assert_eq!(room.players[1].state, PlayerState::Unready);
    assert_eq!(room.state, RoomState::Open);
    players[1].send(&ClientMessage::Ready { ready: true });
    same_update(&mut players).await;

    // Everyone ready and the creator presses: Countdown reaches every client.
    players[0].send(&ClientMessage::Start);
    for client in &mut players {
        let countdown = client.recv().await.unwrap();
        let ServerMessage::Countdown { track, .. } = countdown else {
            panic!("expected Countdown, got {countdown:?}");
        };
        assert_eq!(track, "brands-patch");
    }
    let room = same_update(&mut players).await;
    assert_eq!(room.state, RoomState::Countdown);
    assert!(room
        .players
        .iter()
        .all(|player| player.state == PlayerState::Racing));
    assert_eq!(
        harness.rooms().view(&code).unwrap().state,
        RoomState::Countdown
    );
}

#[tokio::test]
async fn joining_is_refused_while_the_room_races() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = room_of(&mut harness, &code, 1).await;
    let creator = &mut players[0];
    creator.send(&ClientMessage::Ready { ready: true });
    creator.next_room().await.unwrap();
    creator.send(&ClientMessage::Start);
    assert!(matches!(
        creator.recv().await,
        Some(ServerMessage::Countdown { .. })
    ));

    let mut late = harness.connect();
    assert_eq!(late.join(&code, "late").await, Err(Refusal::RoomRacing));
}

#[tokio::test]
async fn the_creator_role_passes_to_the_next_player_when_the_creator_leaves() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = room_of(&mut harness, &code, 3).await;

    // On purpose, with a Leave.
    let creator = players.remove(0);
    creator.send(&ClientMessage::Leave);
    let room = players[0].next_room().await.unwrap();
    assert_eq!(
        room.players
            .iter()
            .map(|player| (player.slot, player.creator))
            .collect::<Vec<_>>(),
        vec![(1, true), (2, false)]
    );
    players[1].next_room().await.unwrap();

    // By dropping the connection.
    players.remove(0).disconnect();
    let room = players[0].next_room().await.unwrap();
    assert_eq!(
        room.players
            .iter()
            .map(|player| (player.slot, player.creator))
            .collect::<Vec<_>>(),
        vec![(2, true)]
    );

    // The freed seats, and their liveries, go to the next to join.
    let mut newcomer = harness.connect();
    let (slot, room) = newcomer.join(&code, "new").await.unwrap();
    assert_eq!(slot, 0);
    assert_eq!(room.players[0].livery, 0);
    assert!(
        !room.players[0].creator,
        "a newcomer never takes the creator role"
    );
}

#[tokio::test]
async fn a_player_who_drops_mid_race_is_disconnected_and_their_kart_stays() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = room_of(&mut harness, &code, 2).await;
    for player in &players {
        player.send(&ClientMessage::Ready { ready: true });
    }
    for player in &mut players {
        player.next_room().await.unwrap();
        player.next_room().await.unwrap();
    }
    players[0].send(&ClientMessage::Start);
    for player in &mut players {
        assert!(matches!(
            player.recv().await,
            Some(ServerMessage::Countdown { .. })
        ));
        player.next_room().await.unwrap();
    }

    players.remove(0).disconnect();
    let room = players[0].next_room().await.unwrap();
    assert_eq!(room.state, RoomState::Countdown);
    assert_eq!(room.players.len(), 2, "the kart stays for a bot to drive");
    assert_eq!(room.players[0].state, PlayerState::Disconnected);
    assert!(room.players[1].creator, "the creator role moves on");
}

#[tokio::test(start_paused = true)]
async fn a_room_enters_closing_five_minutes_after_the_last_human_leaves() {
    let close_after = Duration::from_secs_f32(ROOM_CLOSE_AFTER_EMPTY_S);
    assert_eq!(close_after, Duration::from_secs(300));
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut watched = harness.rooms().watch(&code).unwrap();
    let mut players = room_of(&mut harness, &code, 1).await;

    // Empty for most of the time, then someone comes back: the clock resets.
    players.remove(0).disconnect();
    tokio::time::sleep(close_after - Duration::from_secs(1)).await;
    let mut returning = harness.connect();
    returning.join(&code, "back").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(harness.rooms().view(&code).unwrap().state, RoomState::Open);

    returning.send(&ClientMessage::Leave);
    tokio::time::sleep(close_after - Duration::from_secs(1)).await;
    assert_eq!(harness.rooms().view(&code).unwrap().state, RoomState::Open);
    tokio::time::sleep(Duration::from_secs(2)).await;
    watched.changed().await.unwrap();
    assert_eq!(watched.borrow().state, RoomState::Closing);
    assert_eq!(harness.rooms().view(&code), None, "a closed code is gone");

    let mut late = harness.connect();
    assert_eq!(late.join(&code, "late").await, Err(Refusal::UnknownRoom));
}

#[tokio::test(start_paused = true)]
async fn a_room_nobody_joins_closes_too() {
    let harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    tokio::time::sleep(Duration::from_secs_f32(ROOM_CLOSE_AFTER_EMPTY_S) + Duration::from_secs(1))
        .await;
    assert_eq!(harness.rooms().view(&code), None);
}

#[tokio::test]
async fn codes_come_from_the_unambiguous_alphabet_and_never_repeat_while_open() {
    for ambiguous in ['I', 'L', 'O', '0', '1'] {
        assert!(!ROOM_CODE_ALPHABET.contains(ambiguous), "{ambiguous}");
    }
    assert!(ROOM_CODE_ALPHABET
        .chars()
        .all(|letter| letter.is_ascii_uppercase()));

    let harness = Harness::new(Conditions::PERFECT);
    let mut codes: Vec<String> = (0..200).map(|_| harness.create_room()).collect();
    for code in &codes {
        assert_eq!(code.len(), ROOM_CODE_LENGTH);
        assert!(
            code.chars()
                .all(|letter| ROOM_CODE_ALPHABET.contains(letter)),
            "{code}"
        );
    }
    let mut open = harness.rooms().codes();
    codes.sort();
    open.sort();
    assert_eq!(open, codes);
    codes.dedup();
    assert_eq!(codes.len(), 200);
}

#[tokio::test(start_paused = true)]
async fn the_room_works_the_same_over_a_slow_jittery_link() {
    let mut harness = Harness::new(Conditions {
        delay: Duration::from_millis(100),
        jitter: Duration::from_millis(40),
        loss: 0.0,
        seed: 7,
    });
    let code = harness.create_room();
    let mut players = room_of(&mut harness, &code, 3).await;
    for player in &players {
        player.send(&ClientMessage::Ready { ready: true });
    }
    // Room updates arrive in the order the room made them, however the
    // delay varies, so the last one every client sees has all three ready.
    for player in &mut players {
        let mut last = None;
        for _ in 0..3 {
            last = player.next_room().await;
        }
        assert!(last
            .unwrap()
            .players
            .iter()
            .all(|player| player.state == PlayerState::Ready));
    }
}

#[tokio::test(start_paused = true)]
async fn loss_on_the_fake_link_drops_messages() {
    let mut harness = Harness::new(Conditions {
        delay: Duration::ZERO,
        jitter: Duration::ZERO,
        loss: 1.0,
        seed: 1,
    });
    let code = harness.create_room();
    let mut client = harness.connect();
    client.send(&ClientMessage::Join {
        version: PROTOCOL_VERSION,
        room: code,
        nickname: "khiet".to_owned(),
    });
    assert_eq!(client.recv().await, None);
}

#[tokio::test]
async fn stopping_the_server_ends_every_session() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = room_of(&mut harness, &code, 2).await;
    harness.stop();
    for player in &mut players {
        assert_eq!(player.recv().await, None);
    }
}
