//! The binary as Docker runs it: it listens where LISTEN_ADDR says, answers
//! health, and exits cleanly on SIGTERM. A container restart stays under
//! ten seconds only because that exit is prompt.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn listens_on_the_configured_address_and_exits_on_sigterm() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_server"))
        .env("LISTEN_ADDR", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut ready = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    let addr = ready.trim().rsplit(' ').next().unwrap();

    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");

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
