//! The binary as Docker runs it: it listens where LISTEN_ADDR says, answers
//! health, terminates QUIC when TLS_CERT and TLS_KEY name Caddy's files, and
//! exits cleanly on SIGTERM. A container restart stays under ten seconds
//! only because that exit is prompt.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use server::probe::webtransport_round_trip;
use wtransport::{ClientConfig, Identity};

/// Reads stdout lines until one starts with `prefix` and returns its last word.
fn wait_for_line(stdout: &mut BufReader<ChildStdout>, prefix: &str) -> String {
    loop {
        let mut line = String::new();
        assert!(
            stdout.read_line(&mut line).unwrap() > 0,
            "stdout ended before {prefix:?}"
        );
        if line.starts_with(prefix) {
            return line.trim().rsplit(' ').next().unwrap().to_string();
        }
    }
}

fn health_answers(addr: &str) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
}

fn exits_promptly_on_sigterm(mut child: Child) {
    let sent = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "server still running 2 s after SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "{status}");
}

#[test]
fn listens_on_the_configured_address_and_exits_on_sigterm() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_server"))
        .env("LISTEN_ADDR", "127.0.0.1:0")
        .env_remove("TLS_CERT")
        .env_remove("TLS_KEY")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let addr = wait_for_line(&mut stdout, "server ");
    health_answers(&addr);
    wait_for_line(&mut stdout, "webtransport off");
    exits_promptly_on_sigterm(child);
}

#[tokio::test]
async fn terminates_quic_with_the_configured_certificate_and_exits_on_sigterm() {
    let dir = tempfile::tempdir().unwrap();
    let certificate = dir.path().join("site.crt");
    let private_key = dir.path().join("site.key");
    let identity = Identity::self_signed(["localhost", "127.0.0.1"]).unwrap();
    let hash = identity.certificate_chain().as_slice()[0].hash();
    identity
        .certificate_chain()
        .store_pemfile(&certificate)
        .await
        .unwrap();
    identity
        .private_key()
        .store_secret_pemfile(&private_key)
        .await
        .unwrap();
    let port = UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_server"))
        .env("LISTEN_ADDR", "127.0.0.1:0")
        .env("WEBTRANSPORT_ADDR", port.to_string())
        .env("TLS_CERT", &certificate)
        .env("TLS_KEY", &private_key)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let quic = wait_for_line(&mut stdout, "webtransport listening on ");
    assert_eq!(quic, port.to_string());

    let config = ClientConfig::builder()
        .with_bind_default()
        .with_server_certificate_hashes([hash])
        .build();
    let rtt = webtransport_round_trip(
        config,
        &format!("https://{quic}/wt"),
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert!(rtt < Duration::from_secs(1), "{rtt:?}");
    exits_promptly_on_sigterm(child);
}
