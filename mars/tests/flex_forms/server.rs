/** A disposable form server records complete ordered POST bodies and returned session cookies. */
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/** Each synthetic server owns its listener, request log and bounded cleanup signal. */
pub(super) struct FormServer {
    pub(super) origin: String,
    pub(super) records: Arc<Mutex<Vec<String>>>,
    accepted: std::sync::mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl FormServer {
    /** The response includes cookie state only after the actual submission reaches the wire. */
    pub(super) fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let records = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let observed = records.clone();
        let signal = stop.clone();
        let (accepted, ready) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || serve(listener, &signal, &observed, &accepted));
        Self {
            origin,
            records,
            accepted: ready,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for FormServer {
    /** Fixture lifetime never strands the accept thread or a test socket. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            worker.join().unwrap();
        }
    }
}

/** Nonblocking accept preserves prompt teardown when a product request fails. */
fn serve(
    listener: TcpListener,
    stop: &AtomicBool,
    records: &Mutex<Vec<String>>,
    accepted: &std::sync::mpsc::Sender<()>,
) {
    while !stop.load(Ordering::Relaxed) {
        if let Ok((mut socket, _)) = listener.accept() {
            // Accepted macOS sockets inherit the nonblocking listener flag; reads must wait for request bytes.
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            let _ = accepted.send(());
            if let Some(request) = read_request(&mut socket) {
                records.lock().unwrap().push(request.clone());
                let _ = socket.write_all(response(&request).as_bytes());
            }
        }
        thread::park_timeout(Duration::from_millis(2));
    }
}

/** Read declared request bodies completely even when TCP separates headers and fields. */
fn read_request(socket: &mut std::net::TcpStream) -> Option<String> {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 1024];
        let count = socket.read(&mut chunk).ok()?;
        if count == 0 || bytes.len() + count > 32768 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or_default();
            if bytes.len() >= end + 4 + length {
                return String::from_utf8(bytes).ok();
            }
        }
    }
}

/** Repeated successful field names preserve source order across flex wrapper boundaries. */
fn form() -> String {
    format!(
        "<style>{}</style><form method=post action=/send><div class=row><div class=field><label for=first>First</label><input id=first name=entry value=Ada></div><div class=field><label for=second>Second</label><input id=second name=entry value=Lin></div><div class=field><label for=password>Password</label><input id=password type=password name=password value=private-wire-only required></div></div><input type=hidden name=token value=synthetic-token><button id=send name=intent value=save>Save</button></form>",
        ".row{display:flex;flex-wrap:wrap;gap:16px}.field{display:flex;flex-direction:column;flex:1 1 280px;min-width:0;gap:8px}input{box-sizing:border-box;width:100%}"
    )
}

/** Direct POST results deliberately keep the no-replay history policy observable. */
fn response(request: &str) -> String {
    let (body, cookie) = if request.starts_with("POST ") {
        (
            "<h1>Saved</h1>".into(),
            "Set-Cookie: flex_session=active; Path=/; HttpOnly\r\n",
        )
    } else {
        (form(), "")
    };
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n{cookie}\r\n{body}",
        body.len()
    )
}

/** A ready accepted connection waits for future header bytes instead of treating WouldBlock as a missing request. */
#[test]
fn accepted_connections_wait_for_request_bytes() {
    let server = FormServer::start();
    let mut socket =
        std::net::TcpStream::connect(server.origin.trim_start_matches("http://")).unwrap();
    server
        .accepted
        .recv_timeout(Duration::from_millis(500))
        .unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(25)))
        .unwrap();
    let error = socket.peek(&mut [0; 1]).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    socket
        .write_all(b"GET / HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n")
        .unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
}
