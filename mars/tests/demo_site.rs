use mars::constants;
/** The actual owned demo adapter must release a slow local peer during window shutdown. */
use mars::protocol;
#[path = "../src/desktop/site.rs"]
mod site;
use std::{
    io::Write,
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/** Continuous partial request bytes cannot extend a cancelled owner's lifetime. */
#[test]
fn shutdown_interrupts_a_peer_dripping_incomplete_headers() {
    let site = site::Site::start().unwrap();
    let mut socket = TcpStream::connect(site.origin.trim_start_matches("http://")).unwrap();
    socket
        .set_write_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    socket.write_all(b"G").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let dripper = thread::spawn(move || {
        for _ in 0..20 {
            if signal.load(Ordering::Relaxed) || socket.write_all(b"x").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
    thread::sleep(Duration::from_millis(150));
    let started = Instant::now();
    drop(site);
    let elapsed = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    dripper.join().unwrap();
    assert!(
        elapsed < Duration::from_millis(500),
        "shutdown waited {elapsed:?}"
    );
}

/** An absolute deadline frees the demo for another navigation even without owner cancellation. */
#[test]
fn incomplete_headers_cannot_hold_the_demo_past_the_total_deadline() {
    use std::io::Read;
    let site = site::Site::start().unwrap();
    let address = site.origin.trim_start_matches("http://");
    let mut slow = TcpStream::connect(address).unwrap();
    slow.write_all(b"G").unwrap();
    let dripper = thread::spawn(move || {
        for _ in 0..20 {
            if slow.write_all(b"x").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
    thread::sleep(Duration::from_millis(50));
    let mut next = TcpStream::connect(address).unwrap();
    next.set_read_timeout(Some(Duration::from_millis(900)))
        .unwrap();
    next.write_all(b"GET / HTTP/1.1\r\nHost: fixture\r\n\r\n")
        .unwrap();
    let started = Instant::now();
    let mut response = Vec::new();
    next.read_to_end(&mut response).unwrap();
    let elapsed = started.elapsed();
    dripper.join().unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200 OK"));
    assert!(elapsed < Duration::from_millis(900));
}
