//! The server's transports: the HTTP surface with the WebSocket fallback,
//! served by [`serve`], and WebTransport in [`webtransport`]. `main` binds
//! both; tests drive the router directly or through a socket.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::watch;

pub mod fake_transport;
pub mod probe;
pub mod room;
pub mod rooms;
pub mod session;
mod websocket;
pub mod webtransport;

pub use rooms::Rooms;

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

#[derive(Serialize)]
struct Version {
    commit: &'static str,
}

/// The answer to a room create: the code the creator shares (ADR 0012).
#[derive(Serialize)]
struct CreatedRoom {
    code: String,
}

async fn create_room(State(state): State<AppState>) -> (StatusCode, Json<CreatedRoom>) {
    let code = state.rooms.create();
    (StatusCode::CREATED, Json(CreatedRoom { code }))
}

/// What every route shares: the rooms and the stop signal.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) rooms: Rooms,
    pub(crate) shutdown: Shutdown,
}

/// Every route the server answers; `commit` is what the version endpoint
/// reports and `rooms` is where a create puts a room and a session finds
/// one. Sessions end when `shutdown` resolves.
pub fn app(commit: &'static str, rooms: Rooms, shutdown: Shutdown) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route(
            "/api/version",
            get(move || async move { Json(Version { commit }) }),
        )
        .route("/api/rooms", post(create_room))
        .route("/api/ws", get(websocket::upgrade))
        .with_state(AppState { rooms, shutdown })
}

/// Serve `app` on `listener` until `shutdown` resolves, then finish the
/// requests in flight and return. Idle connections are closed rather than
/// waited on, so the return is prompt; a container restart depends on that.
pub async fn serve(listener: TcpListener, app: Router, shutdown: Shutdown) -> std::io::Result<()> {
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown.wait().await })
        .await
}

/// Tells every long-lived task that the server is stopping. Cheap to clone;
/// every clone resolves together.
#[derive(Clone, Debug)]
pub struct Shutdown {
    stopping: watch::Receiver<bool>,
}

impl Shutdown {
    /// A trigger and the handle it fires: send `true` to stop the server.
    pub fn pair() -> (watch::Sender<bool>, Self) {
        let (sender, stopping) = watch::channel(false);
        (sender, Self { stopping })
    }

    /// Resolves once the server is stopping, at once if it already is.
    pub async fn wait(&self) {
        let mut stopping = self.stopping.clone();
        // A dropped sender counts as stopping: nothing could ever fire it.
        let _ = stopping.wait_for(|stopping| *stopping).await;
    }
}
