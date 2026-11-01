/** A slow link loads: requests fail when the connection stalls, never for taking long. */
use mars_relay::{Prepared, WebRequest};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// Pieces the slow server sends the body in.
const PIECES: usize = 10;
/// Pause between pieces; PIECES of them take longer than the old fixed 8 s request budget.
const PAUSE: Duration = Duration::from_millis(1100);
/// What a caller with no load-time cap passes: the per-request ceiling, in milliseconds.
const CEILING_MS: u64 = 600_000;

/** Serve one response whose body arrives a little at a time; returns the fixture origin. */
fn slow_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut head = [0u8; 4096];
        let _ = stream.read(&mut head);
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {PIECES}\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(header.as_bytes());
        for _ in 0..PIECES {
            std::thread::sleep(PAUSE);
            let _ = stream.write_all(b"x");
            let _ = stream.flush();
        }
    });
    origin
}

/** A body that keeps arriving loads in full even when the whole transfer takes over 8 s. */
#[test]
fn a_body_that_keeps_moving_loads_however_long_it_takes() {
    let origin = slow_server();
    let started = Instant::now();
    let response = Prepared::connect(
        WebRequest::get(&format!("{origin}/slow")),
        std::slice::from_ref(&origin),
        CEILING_MS,
    )
    .and_then(Prepared::transmit)
    .expect("a slow but moving response loads");
    assert_eq!(response.source, "x".repeat(PIECES));
    assert!(started.elapsed() > PAUSE * (PIECES as u32 - 1));
}
