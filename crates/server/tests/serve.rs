//! Serving over TCP and stopping on request, the contract a container
//! restart relies on: Docker sends SIGTERM and waits ten seconds at most.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

#[tokio::test]
async fn answers_over_tcp_then_stops_when_asked() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let served = tokio::spawn(server::serve(listener, server::app("abc"), async {
        stopped.await.ok();
    }));

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");

    stop.send(()).unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(2), served).await;
    outcome
        .expect("serve must return within 2 s of shutdown")
        .unwrap()
        .unwrap();
}
