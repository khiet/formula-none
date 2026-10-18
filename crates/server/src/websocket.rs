//! The WebSocket fallback endpoint: one reliable channel carrying the same
//! bytes WebTransport would (ADR 0011). Caddy proxies it with the rest of
//! `/api`, so it rides on the TLS Caddy terminates.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;

use crate::{ping, Shutdown};

pub(crate) async fn upgrade(ws: WebSocketUpgrade, State(shutdown): State<Shutdown>) -> Response {
    ws.on_upgrade(move |socket| session(socket, shutdown))
}

/// Answer the client until it leaves or the server stops. A stopping server
/// closes the session itself, so a deploy never waits on a player.
async fn session(mut socket: WebSocket, shutdown: Shutdown) {
    loop {
        tokio::select! {
            () = shutdown.wait() => {
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
            received = socket.recv() => match received {
                Some(Ok(Message::Binary(bytes))) => {
                    if let Some(reply) = ping::reply(&bytes) {
                        if socket.send(Message::Binary(reply.into())).await.is_err() {
                            return;
                        }
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                Some(Ok(_)) => {}
            }
        }
    }
}
