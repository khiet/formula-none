//! The server's transports: the HTTP surface with the WebSocket fallback,
//! served by [`serve`], and WebTransport in [`webtransport`]. `main` binds
//! both; tests drive the router directly or through a socket.

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::watch;

mod ping;
pub mod probe;
mod websocket;
pub mod webtransport;

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

/// Every route the server answers; `commit` is what the version endpoint
/// reports. Sessions end when `shutdown` resolves.
pub fn app(commit: &'static str, shutdown: Shutdown) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route(
            "/api/version",
            get(move || async move { Json(Version { commit }) }),
        )
        .route("/api/ws", get(websocket::upgrade))
        .with_state(shutdown)
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
