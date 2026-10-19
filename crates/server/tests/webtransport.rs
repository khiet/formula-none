//! WebTransport (ADR 0011): the server terminates QUIC itself with the
//! certificate files Caddy writes, answers a Ping datagram with a Pong, and
//! picks up a renewed certificate without a restart. Driven through a real
//! QUIC client on loopback.

use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::time::Duration;

use protocol::{encode, ClientMessage, ServerMessage};
use server::probe::webtransport_round_trip;
use server::webtransport::{WebTransportConfig, WebTransportServer};
use server::Shutdown;
use wtransport::tls::Sha256Digest;
use wtransport::{ClientConfig, Endpoint, Identity};

const POLL: Duration = Duration::from_millis(50);

struct Files {
    _dir: tempfile::TempDir,
    certificate: PathBuf,
    private_key: PathBuf,
}

fn files() -> Files {
    let dir = tempfile::tempdir().unwrap();
    Files {
        certificate: dir.path().join("site.crt"),
        private_key: dir.path().join("site.key"),
        _dir: dir,
    }
}

/// Writes a fresh self-signed identity where Caddy would write its files
/// and returns the hash a client pins it by.
async fn write_identity(files: &Files) -> Sha256Digest {
    let identity = Identity::self_signed(["localhost", "127.0.0.1"]).unwrap();
    let chain = identity.certificate_chain();
    chain.store_pemfile(&files.certificate).await.unwrap();
    identity
        .private_key()
        .store_secret_pemfile(&files.private_key)
        .await
        .unwrap();
    chain.as_slice()[0].hash()
}

/// A loopback port nothing listens on right now.
fn free_port() -> SocketAddr {
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

fn config(bind: SocketAddr, files: &Files) -> WebTransportConfig {
    WebTransportConfig {
        bind,
        certificate: files.certificate.clone(),
        private_key: files.private_key.clone(),
        certificate_poll: POLL,
    }
}

/// A registry these sessions never reach: they only Ping.
fn rooms() -> server::Rooms {
    server::Rooms::new(Shutdown::pair().1)
}

fn pinned(hash: &Sha256Digest) -> ClientConfig {
    ClientConfig::builder()
        .with_bind_default()
        .with_server_certificate_hashes([hash.clone()])
        .max_idle_timeout(Some(Duration::from_secs(2)))
        .unwrap()
        .build()
}

fn trusting() -> ClientConfig {
    ClientConfig::builder()
        .with_bind_default()
        .with_no_cert_validation()
        .build()
}

fn url(addr: SocketAddr) -> String {
    format!("https://{addr}/wt")
}

#[tokio::test]
async fn a_ping_datagram_comes_back_as_a_pong_with_its_nonce() {
    let files = files();
    write_identity(&files).await;
    let server = WebTransportServer::bind(config(free_port(), &files), rooms())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let (_stop, shutdown) = Shutdown::pair();
    tokio::spawn(server.run(shutdown));

    let connection = Endpoint::client(trusting())
        .unwrap()
        .connect(url(addr))
        .await
        .unwrap();
    connection
        .send_datagram(encode(&ClientMessage::Ping { nonce: 42 }))
        .unwrap();
    let answer = tokio::time::timeout(Duration::from_secs(2), connection.receive_datagram())
        .await
        .expect("a pong within 2 s")
        .unwrap();
    assert_eq!(
        answer.payload().as_ref(),
        encode(&ServerMessage::Pong { nonce: 42 })
    );
}

#[tokio::test]
async fn the_probe_reports_a_round_trip_over_the_wire() {
    let files = files();
    let hash = write_identity(&files).await;
    let server = WebTransportServer::bind(config(free_port(), &files), rooms())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let (_stop, shutdown) = Shutdown::pair();
    tokio::spawn(server.run(shutdown));

    let rtt = webtransport_round_trip(pinned(&hash), &url(addr), Duration::from_secs(5))
        .await
        .unwrap();
    assert!(rtt < Duration::from_secs(1), "{rtt:?}");
}

#[tokio::test]
async fn the_probe_fails_when_nothing_answers() {
    let addr = free_port();
    let result = webtransport_round_trip(trusting(), &url(addr), Duration::from_millis(500)).await;
    assert!(result.is_err(), "{result:?}");
}

#[tokio::test]
async fn binding_waits_for_caddy_to_write_the_certificate() {
    let files = files();
    let bind = free_port();
    let binding = tokio::spawn(WebTransportServer::bind(config(bind, &files), rooms()));
    tokio::time::sleep(POLL * 4).await;
    assert!(!binding.is_finished(), "bound without a certificate");

    write_identity(&files).await;
    let server = tokio::time::timeout(Duration::from_secs(2), binding)
        .await
        .expect("binds once the files exist")
        .unwrap()
        .unwrap();
    assert_eq!(server.local_addr().unwrap(), bind);
}

#[tokio::test]
async fn a_renewed_certificate_is_served_without_a_restart() {
    let files = files();
    let first = write_identity(&files).await;
    let server = WebTransportServer::bind(config(free_port(), &files), rooms())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let (_stop, shutdown) = Shutdown::pair();
    tokio::spawn(server.run(shutdown));
    webtransport_round_trip(pinned(&first), &url(addr), Duration::from_secs(5))
        .await
        .expect("the first certificate is served");

    // Caddy renews: new files at the same paths.
    let renewed = write_identity(&files).await;
    assert_ne!(renewed, first);
    tokio::time::sleep(POLL * 4).await;

    webtransport_round_trip(pinned(&renewed), &url(addr), Duration::from_secs(5))
        .await
        .expect("the renewed certificate is served");
    let stale = webtransport_round_trip(pinned(&first), &url(addr), Duration::from_secs(3)).await;
    assert!(stale.is_err(), "the old certificate is gone: {stale:?}");
}

#[tokio::test]
async fn a_half_written_renewal_is_skipped_until_the_files_agree() {
    let files = files();
    let first = write_identity(&files).await;
    let server = WebTransportServer::bind(config(free_port(), &files), rooms())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let (_stop, shutdown) = Shutdown::pair();
    let running = tokio::spawn(server.run(shutdown));

    // Caddy has written the new certificate but not yet its key.
    let renewed = Identity::self_signed(["localhost", "127.0.0.1"]).unwrap();
    renewed
        .certificate_chain()
        .store_pemfile(&files.certificate)
        .await
        .unwrap();
    tokio::time::sleep(POLL * 4).await;
    assert!(
        !running.is_finished(),
        "the server survives a mismatched pair"
    );
    webtransport_round_trip(pinned(&first), &url(addr), Duration::from_secs(5))
        .await
        .expect("the first certificate is still served");

    renewed
        .private_key()
        .store_secret_pemfile(&files.private_key)
        .await
        .unwrap();
    tokio::time::sleep(POLL * 4).await;
    let hash = renewed.certificate_chain().as_slice()[0].hash();
    webtransport_round_trip(pinned(&hash), &url(addr), Duration::from_secs(5))
        .await
        .expect("the renewed certificate is served once its key is there");
}

#[tokio::test]
async fn shutdown_ends_the_endpoint() {
    let files = files();
    write_identity(&files).await;
    let server = WebTransportServer::bind(config(free_port(), &files), rooms())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let (stop, shutdown) = Shutdown::pair();
    let running = tokio::spawn(server.run(shutdown));
    let connection = Endpoint::client(trusting())
        .unwrap()
        .connect(url(addr))
        .await
        .unwrap();

    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), running)
        .await
        .expect("run returns within 2 s")
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), connection.closed())
        .await
        .expect("the client sees its session end");
}

/// One message on the reliable stream, framed as the server frames its
/// own: four little-endian length bytes, then the message.
async fn write_frame(stream: &mut wtransport::SendStream, message: &ClientMessage) {
    let bytes = encode(message);
    stream
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .await
        .unwrap();
    stream.write_all(&bytes).await.unwrap();
}

async fn read_frame(stream: &mut wtransport::RecvStream) -> ServerMessage {
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).await.unwrap();
    let mut bytes = vec![0u8; u32::from_le_bytes(length) as usize];
    stream.read_exact(&mut bytes).await.unwrap();
    protocol::decode(&bytes).unwrap()
}

#[tokio::test]
async fn the_room_rides_the_reliable_stream_and_the_snapshots_ride_datagrams() {
    let files = files();
    write_identity(&files).await;
    // The room's task lives as long as the shutdown handle's sender.
    let (_stop, shutdown) = Shutdown::pair();
    let rooms = server::Rooms::new(shutdown.clone());
    let code = rooms.create();
    let server = WebTransportServer::bind(config(free_port(), &files), rooms)
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    tokio::spawn(server.run(shutdown));

    let connection = Endpoint::client(trusting())
        .unwrap()
        .connect(url(addr))
        .await
        .unwrap();
    let (mut send, mut recv) = connection.open_bi().await.unwrap().await.unwrap();
    write_frame(
        &mut send,
        &ClientMessage::Join {
            version: protocol::PROTOCOL_VERSION,
            room: code,
            nickname: "khiet".to_owned(),
        },
    )
    .await;
    let joined = tokio::time::timeout(Duration::from_secs(2), read_frame(&mut recv))
        .await
        .expect("Joined on the stream within 2 s");
    let ServerMessage::Joined { slot: 0, .. } = joined else {
        panic!("expected Joined, got {joined:?}");
    };
    write_frame(&mut send, &ClientMessage::Ready { ready: true }).await;
    write_frame(&mut send, &ClientMessage::Start).await;
    let mut countdown = None;
    while countdown.is_none() {
        match read_frame(&mut recv).await {
            ServerMessage::Countdown { grid, .. } => countdown = Some(grid),
            ServerMessage::Room(_) => {}
            other => panic!("unexpected {other:?} on the stream"),
        }
    }
    assert_eq!(countdown, Some(vec![0]));

    // Snapshots come as datagrams, never on the stream.
    let datagram = tokio::time::timeout(Duration::from_secs(2), connection.receive_datagram())
        .await
        .expect("a snapshot datagram within 2 s")
        .unwrap();
    let message: ServerMessage = protocol::decode(datagram.payload().as_ref()).unwrap();
    assert!(matches!(message, ServerMessage::Snapshot(_)), "{message:?}");
}

#[tokio::test]
async fn a_frame_longer_than_the_tuned_bound_ends_the_session() {
    let files = files();
    write_identity(&files).await;
    let server = WebTransportServer::bind(config(free_port(), &files), rooms())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let (_stop, shutdown) = Shutdown::pair();
    tokio::spawn(server.run(shutdown));

    let connection = Endpoint::client(trusting())
        .unwrap()
        .connect(url(addr))
        .await
        .unwrap();
    let (mut send, _recv) = connection.open_bi().await.unwrap().await.unwrap();
    send.write_all(&(sim::tuning::RELIABLE_FRAME_MAX_BYTES + 1).to_le_bytes())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), connection.closed())
        .await
        .expect("the server closes the session");
}
