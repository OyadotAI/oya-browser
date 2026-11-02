/** Agents attach to one running browser over the private endpoint, each in its own lane. */
use mars::{
    constants::MCP_PROTOCOL_VERSION,
    mcp::attach::{Endpoint, Hub, Protocol, RuntimeHost},
    runtime::{
        EnginePrograms,
        authority::{Authority, Owner, SharedAuthority},
    },
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/** Distinct scratch directories for tests running in parallel. */
static NEXT: AtomicUsize = AtomicUsize::new(0);

/** A page with one text field, served at every path. */
const PAGE: &str = r#"<title>Attach</title><h1>Attach</h1><form action="/"><input id="q" name="q" aria-label="Query"><button>Go</button></form>"#;

/** A loopback site, a headless browser and its attach endpoint in a scratch directory. */
struct Browser {
    origin: String,
    endpoint: Endpoint,
    directory: PathBuf,
    authority: SharedAuthority,
    hub: Option<Hub>,
    cancel: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    site: Option<JoinHandle<()>>,
}

impl Browser {
    /** Start everything; tab-1 belongs to the person. */
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let site = thread::spawn(move || {
            while !signal.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut data = [0; 2048];
                let _ = socket.read(&mut data);
                let _ = socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}", PAGE.len()).as_bytes());
            }
        });
        let directory = std::env::temp_dir().join(format!(
            "mars-a2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let endpoint = Endpoint::in_directory(&directory);
        let authority = Authority::shared(Owner::Human);
        let cancel = Arc::new(AtomicBool::new(false));
        let (listener, token) = endpoint.bind().unwrap();
        let (host, replies, _worker) = RuntimeHost::start(
            EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
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
        Self {
            origin,
            endpoint,
            directory,
            authority,
            hub: Some(hub),
            cancel,
            stop,
            site: Some(site),
        }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.hub.take();
        self.cancel.store(true, Ordering::Release);
        self.stop.store(true, Ordering::Relaxed);
        if let Some(site) = self.site.take() {
            site.join().unwrap();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/** One attached MCP client. */
struct Client {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next: u64,
    lane: String,
}

impl Client {
    /** Attach and finish MCP initialization. */
    fn mcp(browser: &Browser) -> Self {
        let (stream, lane) = browser.endpoint.connect(Protocol::Mcp).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut client = Self {
            stream,
            reader,
            next: 1,
            lane,
        };
        let init = client.request("initialize", json!({"protocolVersion":MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"agent2","version":"1"}}));
        assert!(init.get("result").is_some(), "{init}");
        client.write(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        client
    }
    /** Write one frame. */
    fn write(&mut self, value: Value) {
        self.stream
            .write_all(format!("{value}\n").as_bytes())
            .unwrap();
    }
    /** One request and its response. */
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.write(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    /** One tool call's result object. */
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name":name,"arguments":arguments}))["result"].clone()
    }
}

/** The socket, token and directory are private to this user. */
#[test]
fn endpoint_files_are_private() {
    let browser = Browser::start();
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&browser.directory), 0o700);
    assert_eq!(mode(&browser.endpoint.socket()), 0o600);
    assert_eq!(mode(&browser.endpoint.token_path()), 0o600);
    let second = browser.endpoint.bind();
    assert_eq!(second.unwrap_err().code, "attach_in_use");
}

/** A client without the token is refused before any request runs. */
#[test]
fn wrong_token_is_refused() {
    let browser = Browser::start();
    let mut stream = UnixStream::connect(browser.endpoint.socket()).unwrap();
    stream
        .write_all(b"{\"token\":\"nope\",\"protocol\":\"mcp\"}\n")
        .unwrap();
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).unwrap();
    assert!(answer.contains("\"ok\":false"), "{answer}");
}

/** Two agents attach to the same browser, each opens its own tab, and neither can act on the other's. */
#[test]
fn two_attached_agents_work_in_separate_lanes() {
    let browser = Browser::start();
    let mut first = Client::mcp(&browser);
    let mut second = Client::mcp(&browser);
    assert_ne!(first.lane, second.lane);
    let opened = first.tool("tab_open", json!({"url":format!("{}/", browser.origin)}));
    let tab = opened["structuredContent"]["tab_id"].clone();
    let epoch = opened["structuredContent"]["control_epoch"].clone();
    assert_eq!(
        opened["structuredContent"]["page"]["title"], "Attach",
        "{opened}"
    );
    let summary = second.tool("page_summary", json!({"tab_id":tab}))["structuredContent"].clone();
    let field = summary["controls"][0]["node_id"].clone();
    let intrusion = second.tool("page_type", json!({"tab_id":tab,"control_epoch":epoch,"page_id":summary["page_id"],"expected_revision":summary["revision"],"node_id":field,"text":"x"}));
    assert_eq!(intrusion["isError"], true);
    assert_eq!(
        intrusion["structuredContent"]["error"]["code"], "control_denied",
        "{intrusion} {summary}"
    );
    let typed = first.tool("page_type", json!({"tab_id":tab,"control_epoch":epoch,"page_id":summary["page_id"],"expected_revision":summary["revision"],"node_id":field,"text":"mine"}));
    assert_eq!(typed["isError"], false, "{typed}");
    let shot = first.tool("page_screenshot", json!({"tab_id":tab}));
    assert_eq!(shot["content"][0]["type"], "image");
    assert_eq!(shot["content"][0]["mimeType"], "image/png");
    assert!(shot["structuredContent"].get("png_base64").is_none());
}

/** One agent leaving returns only its tabs to the person; the other keeps working. */
#[test]
fn disconnect_revokes_only_that_lane() {
    let browser = Browser::start();
    let mut first = Client::mcp(&browser);
    let mut second = Client::mcp(&browser);
    let one = first.tool("tab_open", json!({}))["structuredContent"]["tab_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let two = second.tool("tab_open", json!({}))["structuredContent"]["tab_id"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(first);
    let key = |id: &str| id.trim_start_matches("tab-").parse::<u64>().unwrap();
    for _ in 0..200 {
        if browser
            .authority
            .lock()
            .unwrap()
            .tab_status(key(&one))
            .unwrap()
            .0
            == Owner::Human
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        browser
            .authority
            .lock()
            .unwrap()
            .tab_status(key(&one))
            .unwrap()
            .0,
        Owner::Human
    );
    let status = second.tool("control_status", json!({"tab_id":two}));
    assert_eq!(status["structuredContent"]["owner"], "agent");
}

/** Native NDJSON clients attach too; lanes come from the connection, never the client. */
#[test]
fn native_clients_attach_with_their_own_lane() {
    let browser = Browser::start();
    let (mut stream, lane) = browser.endpoint.connect(Protocol::Native).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut call = |value: Value| {
        stream.write_all(format!("{value}\n").as_bytes()).unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };
    let opened = call(json!({"version":2,"id":"open","op":"tab.open","params":{"lane":"spoofed"}}));
    assert_eq!(opened["id"], "open");
    assert_eq!(opened["result"]["lane"], lane, "{opened}");
    let shutdown = call(json!({"version":2,"id":"stop","op":"runtime.shutdown","params":{}}));
    assert_eq!(shutdown["error"]["code"], "control_denied");
}
