//! The server's HTTP surface. `main` binds a listener and serves [`app`];
//! tests drive the router directly.

use std::future::Future;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;

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

/// Every route the server answers; `commit` is what the version endpoint reports.
pub fn app(commit: &'static str) -> Router {
    Router::new().route("/api/health", get(health)).route(
        "/api/version",
        get(move || async move { Json(Version { commit }) }),
    )
}

/// Serve `app` on `listener` until `shutdown` resolves, then finish the
/// connections in flight and return. Idle connections are closed, so a
/// SIGTERM turns into an exit well inside Docker's ten-second grace period.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
}
