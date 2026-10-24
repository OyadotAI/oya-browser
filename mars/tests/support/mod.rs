/** Disposable loopback fixtures and bounded subprocess helpers for native tests. */
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/** Polls never block fixture cleanup or process supervision indefinitely. */
pub const POLL: Duration = Duration::from_millis(5);
/** Allows engine deadlines to expire while bounding every test child. */
pub const COMPLETION: Duration = Duration::from_secs(12);
/** Prevents a failed client from stranding the test fixture. */
const SOCKET_TIMEOUT: Duration = Duration::from_millis(500);
/** Longer than the engine's absolute network deadline. */
const SLOW_RESPONSE: Duration = Duration::from_secs(4);

/** Builds each fixture response from the raw request text. */
type Responder = Box<dyn Fn(&str) -> Vec<u8> + Send>;

/** A disposable listener serves only synthetic in-memory data. */
pub struct Fixture {
    /** URL is always an explicit literal loopback endpoint. */
    pub url: String,
    /** Observes follow-up requests without introducing external networking. */
    pub requests: Arc<AtomicUsize>,
    /** Latest synthetic request supports assertions on replayed session cookies. */
    pub last_request: Arc<std::sync::Mutex<String>>,
    /** The owner can interrupt the nonblocking accept loop. */
    stop: Arc<AtomicBool>,
    /** Every test joins its fixture thread. */
    worker: Option<JoinHandle<()>>,
}

impl Fixture {
    /** Different routes exercise complete, failing and partially valid navigations. */
    pub fn pages() -> Self {
        Self::start(Box::new(page_response), false)
    }
    /** An exact raw response enables malformed HTTP framing tests. */
    pub fn raw(response: Vec<u8>) -> Self {
        Self::start(Box::new(move |_| response.clone()), false)
    }
    /** Exact request paths map to exact raw responses; anything else is a 404. */
    pub fn routes(routes: Vec<(&'static str, Vec<u8>)>) -> Self {
        Self::start(
            Box::new(move |request| {
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                routes
                    .iter()
                    .find(|(route, _)| *route == path)
                    .map(|(_, response)| response.clone())
                    .unwrap_or_else(|| {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_vec()
                    })
            }),
            false,
        )
    }
    /** A held-open socket tests the client's total deadline. */
    pub fn slow() -> Self {
        Self::start(Box::new(page_response), true)
    }
    /** No fixture touches a user profile, filesystem page or external service. */
    fn start(respond: Responder, slow: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let last_request = Arc::new(std::sync::Mutex::new(String::new()));
        let signal = Arc::clone(&stop);
        let count = Arc::clone(&requests);
        let observed = Arc::clone(&last_request);
        let worker =
            thread::spawn(move || serve(listener, respond, slow, &signal, &count, &observed));
        Self {
            url,
            requests,
            last_request,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for Fixture {
    /** Cleanup also wakes a deliberately slow fixture. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            worker.join().unwrap();
        }
    }
}

/** This server exists exclusively for hermetic protocol integration tests. */
fn serve(
    listener: TcpListener,
    respond: Responder,
    slow: bool,
    stop: &AtomicBool,
    requests: &AtomicUsize,
    last_request: &std::sync::Mutex<String>,
) {
    while !stop.load(Ordering::Relaxed) {
        if let Ok((mut socket, _)) = listener.accept() {
            requests.fetch_add(1, Ordering::Relaxed);
            socket.set_nonblocking(false).unwrap();
            socket.set_write_timeout(Some(SOCKET_TIMEOUT)).unwrap();
            let Ok(bytes) = read_request(&mut socket) else {
                continue;
            };
            if slow {
                thread::park_timeout(SLOW_RESPONSE);
                continue;
            }
            let request = String::from_utf8_lossy(&bytes);
            if let Ok(mut observed) = last_request.lock() {
                *observed = request.to_string();
            }
            let response = respond(&request);
            let _ = socket.write_all(&response);
        }
        thread::park_timeout(POLL);
    }
}

/** A fixture must drain a complete bounded header even when TCP splits request delivery. */
fn read_request(socket: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    use mars::constants::{HEADER_BYTES, HEADER_END, NETWORK_CHUNK_BYTES};
    let deadline = Instant::now() + SOCKET_TIMEOUT;
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
    Ok(request)
}

/** Every route is synthetic and has deterministic browser semantics. */
fn page_response(request: &str) -> Vec<u8> {
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    let body = match path {
        "/set-cookie" => return response_with_cookie("<p>candidate</p>", "session=private-value; Path=/; SameSite=Lax"),
        "/with-image" => return response("200 OK", "text/html", "<p>before</p><img id=hero src='/pixel.png' alt='Red pixel'><p>after</p>", None),
        "/inline-image" => return response("200 OK", "text/html", "<p>before<img id=inline src='/pixel.png' alt='Red pixel'>after</p>", None),
        "/table" => return response("200 OK", "text/html", "<table id=prices><caption>Quarterly report</caption><thead><tr><th id=item scope=col>Item</th><th id=amount scope=col>Amount</th></tr></thead><tbody><tr><th id=tea scope=row>Tea</th><td id=tea-amount>A very long amount cell that should wrap when the viewport narrows</td></tr><tr><th id=coffee scope=row>Coffee</th><td id=coffee-amount>7<button>Submit</button></td></tr></tbody><tfoot><tr><th id=total scope=row>Total</th><td id=total-amount>10</td></tr></tfoot></table>", None),
        "/bad-table" => return response("200 OK", "text/html", "<td id=orphan>Orphan</td>", None),
        "/inert-span" => return response("200 OK", "text/html", "<div colspan=2>Text</div>", None),
        "/hidden-table" => return response("200 OK", "text/html", "<table><tbody style='display:none'><tr><td id=hidden>secret</td></tr></tbody><tbody><tr><td id=visible>shown</td></tr></tbody></table>", None),
        "/empty-header-table" => return response("200 OK", "text/html", "<table><thead><tr></tr><tr><th id=column>Header</th></tr></thead><tbody><tr><td id=value>Value</td></tr></tbody></table>", None),
        "/missing-image" => return response("200 OK", "text/html", "<p>before</p><img id=missing src='/missing.png' alt='Missing picture'><p>after</p>", None),
        "/pixel.png" => return binary_response("image/png", &pixel_png()),
        "/missing.png" => return binary_response("image/png", &[]),
        "/script" => include_str!("../../fixtures/hello.html").to_owned(),
        "/badscript" => "<html><body><h1 id=\"title\">candidate</h1><script>unsupported();</script></body></html>".into(),
        "/atomic" => "<html><body><h1 id=\"title\">candidate</h1><script>document.getElementById('title').textContent = 'changed'; document.getElementById('missing').textContent = 'wrong';</script></body></html>".into(),
        "/over-budget" => "<p></p>".repeat(mars::constants::NODES + 1),
        "/large" => format!("<html><body><p id=\"a\">{}</p></body></html>", "\u{0}".repeat(mars::constants::HTML_BYTES / 2)),
        "/ordered" => "<p id=out>initial</p><button id=show onclick='show()'>Show</button><script src='/external.js'></script><script>value=value+1;</script><script>document.getElementById('out').textContent=value;</script>".into(),
        "/inline" => "<p id=out>initial</p><script>let value=6;</script><script>value=value+2;</script><script>document.getElementById('out').textContent=value;</script>".into(),
        "/later-failure" => "<p id=out>candidate</p><script>document.getElementById('out').textContent='partial';</script><script>let value=missing;</script>".into(),
        "/redirect-script" => "<p id=out>candidate</p><script src='/redirect.js'></script>".into(),
        "/wrong-mime" => "<p id=out>candidate</p><script src='/wrong.js'></script>".into(),
        "/wrong-query" => "<p id=out>candidate</p><script src='/wrong.js?token=private-test'></script>".into(),
        "/missing-script" => "<p id=out>candidate</p><script src='/missing.js'></script>".into(),
        "/external.js" => return response("200 OK", "text/javascript; charset=utf-8", "let value=10; function show(){document.getElementById('out').textContent=value;}", None),
        "/wrong.js" => return response("200 OK", "text/plain", "let value=10;", None),
        "/missing.js" => return response("404 Not Found", "text/javascript", "", None),
        "/redirect.js" => return response("302 Found", "text/javascript", "", Some("/target.js")),
        "/target.js" => return response("200 OK", "text/javascript", "let followed=true;", None),
        _ => "<html><body><h1 id=\"title\">original</h1></body></html>".into(),
    };
    response("200 OK", "text/html", &body, None)
}

/** A valid 1×1 RGBA PNG proves binary transport without checked-in opaque data. */
fn pixel_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[255, 0, 0, 255])
        .unwrap();
    bytes
}

/** Binary fixture bodies use exact lengths and never pass through UTF-8 conversion. */
fn binary_response(mime: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    response.extend_from_slice(body);
    response
}

/** Synthetic responses can model script MIME and redirect behavior without external traffic. */
fn response(status: &str, mime: &str, body: &str, location: Option<&str>) -> Vec<u8> {
    let location = location
        .map(|value| format!("Location: {value}\r\n"))
        .unwrap_or_default();
    format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{location}Connection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/** Cookie fixture responses exercise durable candidate commit ordering. */
fn response_with_cookie(body: &str, cookie: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: {cookie}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/** Test processes have a hard completion bound even if product cleanup regresses. */
pub fn finish(mut child: Child) -> Output {
    let deadline = Instant::now() + COMPLETION;
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native test process exceeded deadline");
        }
        thread::sleep(POLL);
    }
}

/** Exercises the public executable with a complete finite agent input stream. */
pub fn agent(input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mars"))
        .arg("agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    finish(child)
}

/** Signal-zero probes only process IDs owned by the test's runtime. */
#[cfg(unix)]
pub fn process_exists(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

/** Parent-only termination may take one bounded engine operation before pipe EOF. */
#[cfg(unix)]
pub fn assert_gone(pids: &[u32]) {
    let deadline = Instant::now() + COMPLETION;
    while pids.iter().any(|pid| process_exists(*pid)) {
        assert!(
            Instant::now() < deadline,
            "owned service survived parent exit: {pids:?}"
        );
        thread::sleep(POLL);
    }
}

/** A complete HTML response with exact framing and any extra header lines the test names. */
pub fn html(extra_headers: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/** A desktop runtime over the owned engines, with only the fixture origin granted. */
pub fn desktop(origin: &str) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** One agent call on tab 1, carrying the current control epoch for mutations. */
pub fn agent_call(
    runtime: &mut mars::runtime::Runtime,
    op: &str,
    mut params: serde_json::Value,
) -> Result<serde_json::Value, mars::protocol::EngineError> {
    use mars::runtime::authority::{Owner, Permit};
    if op.starts_with("page.") {
        params["tab_id"] = serde_json::json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = serde_json::json!(1);
    }
    let permit = Permit {
        owner: Owner::Agent,
        epoch: 1,
        tab_id: Some(1),
    };
    runtime.handle_as(mars::protocol::Request::new("support", op, params), permit)
}
