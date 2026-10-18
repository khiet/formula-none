//! Authoritative server binary: the only place race outcomes are decided.

use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};

/// Set by the image build so the version endpoint names the deployed commit.
const COMMIT: &str = match option_env!("FORMULA_NONE_COMMIT") {
    Some(commit) => commit,
    None => "unknown",
};

/// Resolves on SIGTERM (Docker stop) or SIGINT (a terminal).
async fn shutdown_signal() {
    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut interrupt = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = TcpListener::bind(&addr).await?;
    // The bound address, not the configured one: port 0 resolves here.
    println!("server {COMMIT} listening on {}", listener.local_addr()?);
    server::serve(listener, server::app(COMMIT), shutdown_signal()).await
}
