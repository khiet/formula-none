//! The binary as Docker runs it: it listens where LISTEN_ADDR says, answers
//! health, terminates QUIC when TLS_CERT and TLS_KEY name Caddy's files,
//! logs one JSON object per line (ADR 0018), and exits cleanly on SIGTERM.
//! A container restart stays under ten seconds only because that exit is
//! prompt.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use server::probe::webtransport_round_trip;
use wtransport::{ClientConfig, Identity};

/// Reads stdout lines, each of which must be a JSON object, until one is
/// the event `event`, and returns it.
fn wait_for_event(stdout: &mut BufReader<ChildStdout>, event: &str) -> Value {
    loop {
        let mut line = String::new();
        assert!(
            stdout.read_line(&mut line).unwrap() > 0,
            "stdout ended before {event:?}"
        );
        let parsed: Value =
            serde_json::from_str(&line).unwrap_or_else(|_| panic!("not a JSON line: {line:?}"));
        assert!(parsed.is_object(), "{line:?}");
        if parsed["event"] == event {
            return parsed;
        }
    }
}

/// The string field `key` of a logged line.
fn field(line: &Value, key: &str) -> String {
    line[key]
        .as_str()
        .unwrap_or_else(|| panic!("no string {key:?} in {line}"))
        .to_owned()
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
    let listening = wait_for_event(&mut stdout, "server_listening");
    health_answers(&field(&listening, "addr"));
    wait_for_event(&mut stdout, "webtransport_off");
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
    let quic = field(
        &wait_for_event(&mut stdout, "webtransport_listening"),
        "addr",
    );
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
