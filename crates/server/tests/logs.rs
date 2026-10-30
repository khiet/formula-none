//! The server's log is one JSON object per line (ADR 0018), each naming
//! its event and, where there is one, the room code, the player's slot and
//! the race's tick, so CloudWatch Logs Insights finds a room's lines by
//! its code. Driven through the fake transport, whose server logs into a
//! buffer instead of stdout.

mod common;

use common::race_together;
use protocol::{ClientMessage, PlayerState, Refusal};
use serde_json::Value;
use server::fake_transport::{Conditions, Harness};
use sim::tuning::{COUNTDOWN_S, TICKS_PER_SECOND};

/// The logged lines whose `event` is `event`.
fn lines_of<'a>(lines: &'a [Value], event: &str) -> Vec<&'a Value> {
    lines.iter().filter(|line| line["event"] == event).collect()
}

#[tokio::test(start_paused = true)]
async fn every_line_is_an_object_with_a_timestamp_and_an_event() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut client = harness.connect();
    client.join(&code, "khiet").await.unwrap();
    client.send(&ClientMessage::Leave);
    assert!(client.recv().await.is_none(), "a Leave ends the session");

    let raw = harness.log_text();
    assert!(!raw.is_empty());
    for line in raw.lines() {
        let parsed: Value = serde_json::from_str(line).unwrap_or_else(|error| {
            panic!("not a JSON object: {line:?}: {error}");
        });
        let object = parsed.as_object().expect("an object per line");
        assert!(object["ts"].is_string(), "a timestamp: {line}");
        assert!(object["event"].is_string(), "an event name: {line}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_rooms_lines_carry_its_code_and_a_players_lines_their_slot() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut creator = harness.connect();
    creator.join(&code, "khiet").await.unwrap();
    let mut friend = harness.connect();
    friend.join(&code, "ada").await.unwrap();
    friend.send(&ClientMessage::Leave);
    assert!(friend.recv().await.is_none());
    creator.next_room().await.unwrap();
    creator.next_room().await.unwrap();

    let lines = harness.logs();
    let created = lines_of(&lines, "room_created");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0]["room"], code);

    let joined = lines_of(&lines, "player_joined");
    assert_eq!(joined.len(), 2);
    assert_eq!(joined[0]["room"], code);
    assert_eq!(joined[0]["slot"], 0);
    assert_eq!(joined[0]["nickname"], "khiet");
    assert_eq!(joined[1]["slot"], 1);

    let left = lines_of(&lines, "player_left");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0]["room"], code);
    assert_eq!(left[0]["slot"], 1);
    assert_eq!(left[0]["how"], "leave");
}

#[tokio::test(start_paused = true)]
async fn a_refused_join_is_logged_against_the_code_it_asked_for() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let mut client = harness.connect();
    assert_eq!(
        client.join("zzzz", "khiet").await,
        Err(Refusal::UnknownRoom)
    );

    let lines = harness.logs();
    let refused = lines_of(&lines, "join_refused");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["room"], "ZZZZ");
    assert_eq!(refused[0]["refusal"], "unknownRoom");
}

#[tokio::test(start_paused = true)]
async fn a_races_lines_carry_the_tick() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = vec![harness.connect(), harness.connect()];
    let started = race_together(&code, &mut players).await;
    // Past the countdown: the race is Go.
    let go_tick = u64::from((COUNTDOWN_S * TICKS_PER_SECOND as f32) as u32);
    loop {
        let snapshot = players[0].next_snapshot().await.unwrap();
        if u64::from(snapshot.tick) > go_tick + u64::from(TICKS_PER_SECOND) {
            break;
        }
    }
    // A dropped connection mid-race is logged with the tick it went at,
    // and so is the rejoin that takes the seat back.
    let dropped = players.pop().unwrap();
    let token = dropped.token().unwrap().to_owned();
    dropped.disconnect();
    loop {
        let room = players[0].next_room().await.unwrap();
        if room.players[1].state == PlayerState::Disconnected {
            break;
        }
    }
    let mut back = harness.connect();
    back.rejoin(&code, &token).await.unwrap();

    let lines = harness.logs();
    let race_started = lines_of(&lines, "race_started");
    assert_eq!(race_started.len(), 1);
    assert_eq!(race_started[0]["room"], code);
    assert_eq!(race_started[0]["seed"], started[0].seed);
    assert_eq!(race_started[0]["track"], "oval");

    let go = lines_of(&lines, "race_go");
    assert_eq!(go.len(), 1);
    assert_eq!(go[0]["room"], code);
    assert_eq!(go[0]["tick"], go_tick);

    let left = lines_of(&lines, "player_left");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0]["slot"], 1);
    assert_eq!(left[0]["how"], "dropped");
    assert!(
        left[0]["tick"].as_u64().is_some_and(|tick| tick > go_tick),
        "the tick the player went at: {}",
        left[0]
    );

    let rejoined = lines_of(&lines, "player_rejoined");
    assert_eq!(rejoined.len(), 1);
    assert_eq!(rejoined[0]["slot"], 1);
    assert!(
        rejoined[0]["tick"].as_u64() >= left[0]["tick"].as_u64(),
        "the tick the player came back at: {}",
        rejoined[0]
    );
}
