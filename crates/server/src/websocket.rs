//! The WebSocket fallback endpoint: one reliable channel carrying the same
//! bytes WebTransport would (ADR 0011). Caddy proxies it with the rest of
//! `/api`, so it rides on the TLS Caddy terminates.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use tokio::sync::mpsc;

use crate::session::{self, Channels};
use crate::{AppState, Shutdown};

pub(crate) async fn upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| run(socket, state))
}

/// Pump the socket into a session and the session's answers back until the
/// client leaves, the session ends or the server stops. A stopping server
/// closes the session itself, so a deploy never waits on a player.
async fn run(socket: WebSocket, state: AppState) {
    let (inbound_tx, inbound) = mpsc::unbounded_channel();
    let (outbound, outbound_rx) = mpsc::unbounded_channel();
    let session = tokio::spawn(session::run(
        state.rooms,
        Channels { inbound, outbound },
        state.shutdown.clone(),
    ));
    pump(socket, inbound_tx, outbound_rx, state.shutdown).await;
    session.abort();
}

async fn pump(
    mut socket: WebSocket,
    inbound: mpsc::UnboundedSender<Vec<u8>>,
    mut outbound: mpsc::UnboundedReceiver<Vec<u8>>,
    shutdown: Shutdown,
) {
    loop {
        tokio::select! {
            () = shutdown.wait() => {
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
            // The session dropped its end: it refused the client or the client left.
            sending = outbound.recv() => match sending {
                Some(bytes) => {
                    if socket.send(Message::Binary(bytes.into())).await.is_err() {
                        return;
                    }
                }
                None => {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            },
            received = socket.recv() => match received {
                Some(Ok(Message::Binary(bytes))) => {
                    if inbound.send(bytes.to_vec()).is_err() {
                        return;
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                Some(Ok(_)) => {}
            }
        }
    }
}
