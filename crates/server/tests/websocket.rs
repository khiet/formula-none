//! The WebSocket fallback (ADR 0011): the server's `/api/ws` endpoint
//! carries the protocol's bytes and answers a Ping with a Pong, driven
//! through a real socket the way Caddy proxies a browser's.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use protocol::{encode, ClientMessage, ServerMessage};
use server::Shutdown;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// A server on a free port and a WebSocket connected to it; the sender
/// stops the server.
async fn connected() -> (Socket, tokio::sync::watch::Sender<bool>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, shutdown) = Shutdown::pair();
    let app = server::app("0123abcd", shutdown.clone());
    let serving = tokio::spawn(async move {
        server::serve(listener, app, shutdown).await.unwrap();
    });
    let (socket, _) = connect_async(format!("ws://{addr}/api/ws")).await.unwrap();
    (socket, stop, serving)
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
    // A room message is well formed but has no handler until the rooms land.
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
