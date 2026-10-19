//! The WebSocket fallback (ADR 0011): the server's `/api/ws` endpoint
//! carries the protocol's bytes and answers a Ping with a Pong, driven
//! through a real socket the way Caddy proxies a browser's.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use protocol::{
    decode, encode, ClientMessage, PlayerState, Refusal, ServerMessage, PROTOCOL_VERSION,
};
use server::Shutdown;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// A server on a free port and a WebSocket connected to it; the sender
/// stops the server.
async fn connected() -> (Socket, tokio::sync::watch::Sender<bool>, JoinHandle<()>) {
    let (socket, stop, serving, _) = connected_with_rooms().await;
    (socket, stop, serving)
}

/// As [`connected`], with the server's rooms so a test can create one.
async fn connected_with_rooms() -> (
    Socket,
    tokio::sync::watch::Sender<bool>,
    JoinHandle<()>,
    server::Rooms,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, shutdown) = Shutdown::pair();
    let rooms = server::Rooms::new(shutdown.clone());
    let app = server::app("0123abcd", rooms.clone(), shutdown.clone());
    let serving = tokio::spawn(async move {
        server::serve(listener, app, shutdown).await.unwrap();
    });
    let (socket, _) = connect_async(format!("ws://{addr}/api/ws")).await.unwrap();
    (socket, stop, serving, rooms)
}

async fn next_binary(socket: &mut Socket) -> Vec<u8> {
    loop {
        match tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("an answer within 2 s")
            .expect("the socket stays open")
            .unwrap()
        {
            Message::Binary(bytes) => return bytes.to_vec(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[tokio::test]
async fn a_ping_comes_back_as_a_pong_with_its_nonce() {
    let (mut socket, _stop, _serving) = connected().await;
    let ping = encode(&ClientMessage::Ping { nonce: 42 });
    socket.send(Message::Binary(ping.into())).await.unwrap();
    let answer = next_binary(&mut socket).await;
    assert_eq!(answer, encode(&ServerMessage::Pong { nonce: 42 }));
}

#[tokio::test]
async fn bytes_that_are_not_a_message_the_server_answers_are_ignored_and_the_session_continues() {
    let (mut socket, _stop, _serving) = connected().await;
    socket
        .send(Message::Binary(vec![0xff, 0xff].into()))
        .await
        .unwrap();
    socket.send(Message::text("hello")).await.unwrap();
    // A room message before a Join has no room to go to.
    let start = encode(&ClientMessage::Start);
    socket.send(Message::Binary(start.into())).await.unwrap();
    let ping = encode(&ClientMessage::Ping { nonce: 1 });
    socket.send(Message::Binary(ping.into())).await.unwrap();
    let answer = next_binary(&mut socket).await;
    assert_eq!(answer, encode(&ServerMessage::Pong { nonce: 1 }));
}

#[tokio::test]
async fn shutdown_closes_an_open_session_and_the_server_returns_promptly() {
    let (mut socket, stop, serving) = connected().await;
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), serving)
        .await
        .expect("serve returns within 2 s with a session open")
        .unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(frame) = socket.next().await {
            if matches!(frame, Ok(Message::Close(_)) | Err(_)) {
                return true;
            }
        }
        true
    })
    .await;
    assert_eq!(closed, Ok(true), "the client sees the session end");
}

#[tokio::test]
async fn a_join_over_the_socket_seats_the_player_and_the_session_keeps_answering_pings() {
    let (mut socket, _stop, _serving, rooms) = connected_with_rooms().await;
    let code = rooms.create();
    let join = encode(&ClientMessage::Join {
        version: PROTOCOL_VERSION,
        room: code.clone(),
        nickname: "khiet".to_owned(),
    });
    socket.send(Message::Binary(join.into())).await.unwrap();
    let joined: ServerMessage = decode(&next_binary(&mut socket).await).unwrap();
    let ServerMessage::Joined { slot, room, .. } = joined else {
        panic!("expected Joined, got {joined:?}");
    };
    assert_eq!(slot, 0);
    assert_eq!(room.code, code);
    assert_eq!(room.players[0].nickname, "khiet");

    // Room messages reach the room, and a Ping is still the session's to answer.
    let ready = encode(&ClientMessage::Ready { ready: true });
    socket.send(Message::Binary(ready.into())).await.unwrap();
    let update: ServerMessage = decode(&next_binary(&mut socket).await).unwrap();
    let ServerMessage::Room(room) = update else {
        panic!("expected Room, got {update:?}");
    };
    assert_eq!(room.players[0].state, PlayerState::Ready);
    let ping = encode(&ClientMessage::Ping { nonce: 9 });
    socket.send(Message::Binary(ping.into())).await.unwrap();
    assert_eq!(
        next_binary(&mut socket).await,
        encode(&ServerMessage::Pong { nonce: 9 })
    );
}

#[tokio::test]
async fn a_refused_join_is_answered_and_then_the_server_closes_the_socket() {
    let (mut socket, _stop, _serving) = connected().await;
    let join = encode(&ClientMessage::Join {
        version: PROTOCOL_VERSION,
        room: "ZZZZ".to_owned(),
        nickname: "khiet".to_owned(),
    });
    socket.send(Message::Binary(join.into())).await.unwrap();
    assert_eq!(
        next_binary(&mut socket).await,
        encode(&ServerMessage::Refused(Refusal::UnknownRoom))
    );
    let closed = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(frame) = socket.next().await {
            if matches!(frame, Ok(Message::Close(_)) | Err(_)) {
                return true;
            }
        }
        true
    })
    .await;
    assert_eq!(closed, Ok(true), "the client sees the session end");
}

#[tokio::test]
async fn the_race_flows_over_the_one_channel_too() {
    // The fallback carries both lanes on the socket (ADR 0011): the
    // countdown and the events as binary messages like the snapshots.
    let (mut socket, _stop, _serving, rooms) = connected_with_rooms().await;
    let code = rooms.create();
    for message in [
        ClientMessage::Join {
            version: PROTOCOL_VERSION,
            room: code,
            nickname: "khiet".to_owned(),
        },
        ClientMessage::Ready { ready: true },
        ClientMessage::Start,
    ] {
        socket
            .send(Message::Binary(encode(&message).into()))
            .await
            .unwrap();
    }
    let mut countdown = None;
    let mut snapshot = None;
    let mut input_sequence = 0;
    while countdown.is_none() || snapshot.is_none() {
        let message: ServerMessage = decode(&next_binary(&mut socket).await).unwrap();
        match message {
            ServerMessage::Countdown { grid, .. } => countdown = Some(grid),
            ServerMessage::Snapshot(received) => {
                input_sequence += 1;
                let input = ClientMessage::Input(protocol::Input {
                    sequence: input_sequence,
                    steer: 0,
                    throttle: 0,
                    brake: 0,
                    hop: false,
                    use_item: false,
                    hold_item: false,
                });
                socket
                    .send(Message::Binary(encode(&input).into()))
                    .await
                    .unwrap();
                if received.acknowledged[0] > 0 {
                    snapshot = Some(received);
                }
            }
            ServerMessage::Joined { .. } | ServerMessage::Room(_) | ServerMessage::Events(_) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(countdown, Some(vec![0]));
    assert_eq!(snapshot.unwrap().karts.len(), sim::tuning::KARTS_PER_RACE);
}
