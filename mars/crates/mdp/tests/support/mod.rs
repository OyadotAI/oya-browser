/** A loopback site, a headless Mars runtime, its attach hub and the CDP server, all in-process. */
use mars::{
    mcp::attach::{Endpoint, Hub, RuntimeHost},
    runtime::{
        EnginePrograms,
        authority::{Authority, Owner, SharedAuthority},
    },
};
use mars_mdp::{Attached, Native, server};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

/** The fixture site's response for one request line and path. */
pub type Respond = Arc<dyn Fn(&str) -> String + Send + Sync>;

/** Distinct scratch directories for parallel tests. */
static NEXT: AtomicUsize = AtomicUsize::new(0);

/** Site, headless browser, attach hub and CDP server, all in-process and loopback. */
pub struct Stack {
    pub origin: String,
    pub port: u16,
    pub authority: SharedAuthority,
    directory: PathBuf,
    _hub: Hub,
    cancel: Arc<AtomicBool>,
}

impl Stack {
    /** Start everything; tab-1 belongs to the person. `respond` maps a request path to HTML. */
    pub fn start(respond: Respond) -> Self {
        let site = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", site.local_addr().unwrap());
        thread::spawn(move || {
            for mut socket in site.incoming().flatten() {
                let mut data = [0; 4096];
                let count = socket.read(&mut data).unwrap_or(0);
                let request = String::from_utf8_lossy(&data[..count]);
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let body = respond(&path);
                let (status, mime) = if path.starts_with("/api/") {
                    ("200 OK", "application/json")
                } else if path.starts_with("/missing") {
                    ("404 Not Found", "text/html; charset=utf-8")
                } else {
                    ("200 OK", "text/html; charset=utf-8")
                };
                let _ = socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        let directory = std::env::temp_dir().join(format!(
            "mars-mdp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let endpoint = Endpoint::in_directory(&directory);
        let authority = Authority::shared(Owner::Human);
        let cancel = Arc::new(AtomicBool::new(false));
        let (listener, token) = endpoint.bind().unwrap();
        let (host, replies, _) = RuntimeHost::start(
            EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars-mdp"))),
            vec![origin.clone()],
            authority.clone(),
            cancel.clone(),
        )
        .unwrap();
        let hub = Hub::start(
            listener,
            token,
            host,
            replies,
            authority.clone(),
            cancel.clone(),
        )
        .unwrap();
        let cdp = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = cdp.local_addr().unwrap().port();
        thread::spawn(move || {
            server::serve(
                cdp,
                "browser-id".into(),
                Arc::new(move || {
                    Attached::connect(&endpoint)
                        .map(|native| Box::new(native) as Box<dyn Native + Send>)
                }),
            );
        });
        Self {
            origin,
            port,
            authority,
            directory,
            _hub: hub,
            cancel,
        }
    }
    /** One raw HTTP GET with chosen headers. */
    pub fn get(&self, path: &str, headers: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\n{headers}\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/** A minimal masked WebSocket CDP client. */
pub struct Client {
    stream: TcpStream,
    next: u64,
}

impl Client {
    /** Discover the browser endpoint and upgrade. */
    pub fn connect(stack: &Stack) -> Self {
        let host = format!("Host: 127.0.0.1:{}\r\n", stack.port);
        let version = stack.get("/json/version", &host);
        let body: Value = serde_json::from_str(version.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        let url = body["webSocketDebuggerUrl"].as_str().unwrap().to_owned();
        let path = url.splitn(4, '/').nth(3).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", stack.port)).unwrap();
        write!(stream, "GET /{path} HTTP/1.1\r\n{host}Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n").unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.starts_with("HTTP/1.1 101"), "{line}");
        while line != "\r\n" {
            line.clear();
            reader.read_line(&mut line).unwrap();
        }
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        Self { stream, next: 0 }
    }
    /** Send one masked text frame. */
    fn send(&mut self, value: &Value) {
        let payload = value.to_string().into_bytes();
        let mut frame = vec![0x81];
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        let mask = [1_u8, 2, 3, 4];
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.stream.write_all(&frame).unwrap();
    }
    /** Read one unmasked server frame. */
    fn receive(&mut self) -> Value {
        let mut head = [0_u8; 2];
        self.stream.read_exact(&mut head).unwrap();
        let length = match head[1] & 0x7f {
            126 => {
                let mut bytes = [0_u8; 2];
                self.stream.read_exact(&mut bytes).unwrap();
                u16::from_be_bytes(bytes) as usize
            }
            127 => {
                let mut bytes = [0_u8; 8];
                self.stream.read_exact(&mut bytes).unwrap();
                u64::from_be_bytes(bytes) as usize
            }
            short => short as usize,
        };
        let mut payload = vec![0_u8; length];
        self.stream.read_exact(&mut payload).unwrap();
        serde_json::from_slice(&payload).unwrap()
    }
    /** One command; events before the response are collected and returned with it. */
    pub fn command(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> (Value, Vec<Value>) {
        self.next += 1;
        let mut message = json!({"id":self.next,"method":method,"params":params});
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        self.send(&message);
        let mut events = Vec::new();
        loop {
            let value = self.receive();
            if value["id"] == self.next {
                return (value, events);
            }
            events.push(value);
        }
    }
}
