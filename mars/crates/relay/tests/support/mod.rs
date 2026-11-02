/** A keep-alive loopback HTTP server that records every request and counts connections. */
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/** Every Prepared call in a test gets the whole navigation budget. */
pub const REMAINING_MS: u64 = 8_000;

/** Builds a response for a request head and its zero-based index across all connections. */
pub type Respond = dyn Fn(&str, usize) -> Vec<u8> + Send + Sync;

/** A running server: its origin, recorded request heads, and accepted connection count. */
pub struct Server {
    /// `http://127.0.0.1:<port>`, which is also the fixture grant.
    pub origin: String,
    requests: Arc<Mutex<Vec<String>>>,
    connections: Arc<AtomicUsize>,
}

impl Server {
    /// Serves until the test process exits; each connection runs on its own thread.
    pub fn start(respond: impl Fn(&str, usize) -> Vec<u8> + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
        let origin = format!("http://{}", listener.local_addr().expect("address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let respond: Arc<Respond> = Arc::new(respond);
        let (log, count) = (requests.clone(), connections.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                count.fetch_add(1, Ordering::SeqCst);
                let (log, respond) = (log.clone(), respond.clone());
                std::thread::spawn(move || serve(stream, &log, &*respond));
            }
        });
        Self {
            origin,
            requests,
            connections,
        }
    }

    /// Absolute URL for `path` on this server.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }

    /// The fixture grant list for this server.
    pub fn grants(&self) -> Vec<String> {
        vec![self.origin.clone()]
    }

    /// Request heads received so far.
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("log").clone()
    }

    /// Connections accepted so far.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }
}

/** Answers requests on one connection until the client closes or a response says close. */
fn serve(mut stream: TcpStream, log: &Mutex<Vec<String>>, respond: &Respond) {
    loop {
        let Some(head) = read_head(&mut stream) else {
            return;
        };
        let index = {
            let mut log = log.lock().expect("log");
            log.push(head.clone());
            log.len() - 1
        };
        let response = respond(&head, index);
        if stream.write_all(&response).is_err() {
            return;
        }
        let text = String::from_utf8_lossy(&response).to_ascii_lowercase();
        if text.contains("connection: close") || text.starts_with("http/1.0") {
            return;
        }
    }
}

/** Reads one request head and discards a Content-Length body. */
fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut head = Vec::new();
    let mut byte = [0; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let length = head
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().to_owned())
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).ok()?;
    Some(head)
}

/// A 200 text/html response with extra header lines and a Content-Length body.
pub fn html(extra: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n{extra}\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/// The value of a request header, matched case-insensitively by name.
pub fn header(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (field, value) = line.split_once(':')?;
        field
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}
