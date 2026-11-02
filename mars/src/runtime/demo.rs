/** Self-contained fixture proves the owned engines and native tools work together. */
use super::supervisor::Runtime;
use crate::constants::{HEADER_BYTES, HEADER_END, NETWORK_CHUNK_BYTES, NETWORK_TIMEOUT};
use crate::protocol::{EngineError, EngineResult, Request};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/** Immutable fixture contains a real V9 program, not a scripted host-side mutation. */
const FIXTURE: &str = include_str!("../../fixtures/hello.html");
/** Short bounded parking allows shutdown without a wake-up socket connection. */
const ACCEPT_POLL: Duration = Duration::from_millis(5);

/// Runs real HTTP, parsing, V9, inspection, native mutation and layout in sequence.
pub fn run_demo(runtime: &mut Runtime) -> EngineResult<()> {
    eprintln!("Mars — Rust browser for people and agents\nLoading fixture…");
    let fixture = Fixture::start()?;
    let loaded = tool(runtime, "page.navigate", json!({"url":fixture.url}))?;
    println!("Loaded local fixture.\nV9 updated document text.");
    let inspected = tool(runtime, "page.inspect", json!({}))?;
    show("Native agent inspected the document.", &inspected)?;
    mutate(runtime, &loaded)?;
    show(
        "Native agent updated document text.",
        &tool(runtime, "page.inspect", json!({}))?,
    )?;
    show("Layout updated.", &tool(runtime, "page.layout", json!({}))?)?;
    tool(runtime, "runtime.shutdown", json!({}))?;
    println!("Demo complete.");
    eprintln!("Stopped.");
    Ok(())
}

/** Mutation uses the exact native handle and revision returned by navigation. */
fn mutate(runtime: &mut Runtime, loaded: &Value) -> EngineResult<()> {
    let title = &loaded["nodes"][0];
    if title["text"] != "Hello from V9" {
        return Err(EngineError::invalid(
            "V9 did not produce the expected fixture text.",
        ));
    }
    tool(
        runtime,
        "page.set_text",
        json!({"page_id":loaded["page_id"],"node_id":title["node_id"],"expected_revision":loaded["revision"],"text":"Hello from a native agent"}),
    )?;
    Ok(())
}

/** Demo calls exactly the same operations available to an external native agent. */
fn tool(runtime: &mut Runtime, op: &str, params: Value) -> EngineResult<Value> {
    runtime.handle(Request::new("demo", op, params))
}

/** Page-controlled controls are escaped even when JSON leaves Unicode controls literal. */
fn show(label: &str, value: &Value) -> EngineResult<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|_| EngineError::invalid("Cannot format demo output."))?;
    let safe: String = json
        .chars()
        .flat_map(|character| {
            if character.is_control() && character != '\n' {
                character.escape_default().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect();
    println!("{label}\n{safe}");
    Ok(())
}

/** An owned local listener exists only while the demo is running. */
struct Fixture {
    /** The one literal-loopback URL provided to the runtime. */
    url: String,
    /** Stop signal shared only with the fixture thread. */
    stop: Arc<AtomicBool>,
    /** Joined on success and early error alike. */
    worker: Option<JoinHandle<()>>,
}

impl Fixture {
    /** An ephemeral OS-assigned port avoids interfering with user services. */
    fn start() -> EngineResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(fixture_error)?;
        listener.set_nonblocking(true).map_err(fixture_error)?;
        let url = format!(
            "http://{}/hello",
            listener.local_addr().map_err(fixture_error)?
        );
        let stop = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        let worker = thread::spawn(move || serve(listener, &signal));
        Ok(Self {
            url,
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for Fixture {
    /** A nonblocking listener cannot strand a fixture thread after failed navigation. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}

/** This fixed fixture is not a general server and cannot serve filesystem paths. */
fn serve(listener: TcpListener, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        if let Ok((mut socket, _)) = listener.accept() {
            if socket.set_nonblocking(false).is_err() || read_request(&mut socket).is_err() {
                continue;
            }
            let _ = socket.set_write_timeout(Some(NETWORK_TIMEOUT));
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{FIXTURE}",
                FIXTURE.len()
            );
            let _ = socket.write_all(reply.as_bytes());
        }
        thread::park_timeout(ACCEPT_POLL);
    }
}

/** Accepted sockets may inherit nonblocking mode; complete bounded headers precede replies. */
fn read_request(socket: &mut TcpStream) -> std::io::Result<()> {
    let deadline = Instant::now() + NETWORK_TIMEOUT;
    let mut request = Vec::new();
    while !request
        .windows(HEADER_END.len())
        .any(|part| part == HEADER_END)
    {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        socket.set_read_timeout(Some(remaining))?;
        let mut bytes = [0; NETWORK_CHUNK_BYTES];
        let count = socket.read(&mut bytes)?;
        if count == 0 || request.len() + count > HEADER_BYTES {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        request.extend_from_slice(&bytes[..count]);
    }
    Ok(())
}

/** Demo bind failures are explicit and do not fall back to another runtime. */
fn fixture_error(_: std::io::Error) -> EngineError {
    EngineError::new(
        "network_error",
        "The local demo fixture could not start.",
        "Allow local loopback sockets and run the demo again.",
    )
}
