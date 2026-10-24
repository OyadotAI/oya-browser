/**
 * The loopback HTTP and WebSocket endpoint CDP clients discover: `/json/version`, `/json/list`,
 * `/json/protocol`, `/json/new`, `/json/close`, and WebSocket upgrades at `/devtools/browser/<id>`
 * and `/devtools/page/<tab>`. Like Chrome's
 * remote debugging port it is loopback only; it also refuses foreign Host headers (DNS
 * rebinding) and any request carrying an Origin, so web pages cannot drive it.
 */
use crate::{Native, session::Session, websocket};
use mars::protocol::EngineResult;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::Duration,
};

/// Largest HTTP request head accepted.
const HEAD_BYTES: usize = 8 * 1024;
/// A client that opens a socket and says nothing is dropped this soon.
const HEAD_DEADLINE: Duration = Duration::from_secs(5);

/** Opens one native lane per CDP connection. */
pub type Connect = Arc<dyn Fn() -> EngineResult<Box<dyn Native + Send>> + Send + Sync>;

/** Serve CDP clients on `listener` until the process ends; `id` names the browser endpoint. */
pub fn serve(listener: TcpListener, id: String, connect: Connect) {
    for stream in listener.incoming().flatten() {
        let (id, connect) = (id.clone(), connect.clone());
        thread::spawn(move || {
            let _ = handle(stream, &id, &connect);
        });
    }
}

/** One parsed request head. */
struct Head {
    path: String,
    host: String,
    origin: bool,
    upgrade_key: Option<String>,
}

/** Read and parse the request head. */
fn read_head(stream: &mut TcpStream) -> Option<Head> {
    let _ = stream.set_read_timeout(Some(HEAD_DEADLINE));
    let mut data = Vec::new();
    let mut byte = [0_u8; 1];
    while !data.ends_with(b"\r\n\r\n") {
        if data.len() >= HEAD_BYTES || stream.read(&mut byte).ok()? != 1 {
            return None;
        }
        data.push(byte[0]);
    }
    let text = String::from_utf8(data).ok()?;
    let mut lines = text.split("\r\n");
    let path = lines.next()?.split_whitespace().nth(1)?.to_owned();
    let mut head = Head {
        path,
        host: String::new(),
        origin: false,
        upgrade_key: None,
    };
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => head.host = value.trim().to_owned(),
            "origin" => head.origin = true,
            "sec-websocket-key" => head.upgrade_key = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    Some(head)
}

/** Only loopback names on this port; anything else may be a rebinding attack. */
fn local_host(host: &str, port: u16) -> bool {
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|name| host == format!("{name}:{port}"))
}

/** Route one connection. */
fn handle(mut stream: TcpStream, id: &str, connect: &Connect) -> std::io::Result<()> {
    let port = stream.local_addr()?.port();
    let Some(head) = read_head(&mut stream) else {
        return Ok(());
    };
    if head.origin || !local_host(&head.host, port) {
        return respond(
            &mut stream,
            "403 Forbidden",
            &json!({"error":"Mars accepts CDP clients from this machine only, without an Origin."}),
        );
    }
    let base = format!("ws://{}", head.host);
    // Chrome's handler takes `/json/version/` and `/json/list?for_tab` alike.
    let path = head.path.split('#').next().unwrap_or_default();
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    let route = route.trim_end_matches('/');
    let route = if route.is_empty() { "/" } else { route };
    let path = if query.is_empty() {
        route.to_owned()
    } else {
        format!("{route}?{query}")
    };
    match (path.as_str(), head.upgrade_key.clone()) {
        ("/json/version", _) => respond(
            &mut stream,
            "200 OK",
            &json!({"Browser":mars::network::CHROME_ENGINE,"Protocol-Version":crate::catalog::Catalog::pinned().version,"User-Agent":crate::session::user_agent(),"V8-Version":"V9","WebKit-Version":"Eye","webSocketDebuggerUrl":format!("{base}/devtools/browser/{id}")}),
        ),
        ("/json" | "/json/list", _) => {
            let list = page_list(connect, &base);
            respond(&mut stream, "200 OK", &list)
        }
        ("/json/protocol", _) => respond(&mut stream, "200 OK", &protocol()),
        (path, _) if path.starts_with("/json/new") => {
            let url = query;
            match new_target(connect, &base, url) {
                Ok(descriptor) => respond(&mut stream, "200 OK", &descriptor),
                Err(error) => respond(
                    &mut stream,
                    "500 Internal Server Error",
                    &json!({"error":error}),
                ),
            }
        }
        (path, _) if path.starts_with("/json/activate/") => {
            respond(&mut stream, "200 OK", &json!("Target activated"))
        }
        (path, _) if path.starts_with("/json/close/") => {
            let tab = path.trim_start_matches("/json/close/").to_owned();
            match close_target(connect, &tab) {
                Ok(()) => respond(&mut stream, "200 OK", &json!("Target is closing")),
                Err(error) => respond(
                    &mut stream,
                    "500 Internal Server Error",
                    &json!({"error":error}),
                ),
            }
        }
        (path, Some(key)) if path == format!("/devtools/browser/{id}") => {
            upgrade(stream, &key, connect, None)
        }
        (path, Some(key)) if path.starts_with("/devtools/page/tab-") => {
            let tab = path.trim_start_matches("/devtools/page/").to_owned();
            upgrade(stream, &key, connect, Some(tab))
        }
        _ => respond(
            &mut stream,
            "404 Not Found",
            &json!({"error":"Unknown CDP endpoint."}),
        ),
    }
}

/** `/json/list`: the browser's tabs as page targets. */
fn page_list(connect: &Connect, base: &str) -> Value {
    let tabs = connect()
        .ok()
        .and_then(|mut native| native.call("tab.list", json!({})).ok());
    let tabs = tabs
        .and_then(|tabs| tabs["tabs"].as_array().cloned())
        .unwrap_or_default();
    json!(tabs.iter().map(|tab| {
        let id = tab["tab_id"].as_str().unwrap_or_default();
        json!({"id":id,"type":"page","title":tab["title"],"url":"","description":"","devtoolsFrontendUrl":"","webSocketDebuggerUrl":format!("{base}/devtools/page/{id}")})
    }).collect::<Vec<_>>())
}

/** `/json/protocol`: the pinned protocol with Mars's status on every command. */
fn protocol() -> Value {
    let catalog = crate::catalog::Catalog::pinned();
    let implemented = crate::session::implemented();
    let domains: Vec<Value> = catalog
        .domains
        .iter()
        .map(|domain| {
            let commands: Vec<Value> = domain
                .commands
                .iter()
                .map(|command| {
                    let method = format!("{}.{}", domain.name, command.name);
                    let status = crate::matrix::status(&method, &implemented, crate::session::ACCEPTED);
                    let mut entry = json!({"name":command.name,"parameters":command.fields,"returns":command.returns,"status":status.label()});
                    if let crate::matrix::Status::NotApplicable(reason) = status {
                        entry["reason"] = json!(reason);
                    }
                    entry
                })
                .collect();
            json!({"domain":domain.name,"experimental":domain.experimental,"deprecated":domain.deprecated,"commands":commands,"events":domain.events.iter().map(|event| json!(event.name)).collect::<Vec<_>>()})
        })
        .collect();
    json!({"version":{"major":catalog.version.split('.').next().unwrap_or("1"),"minor":catalog.version.split('.').nth(1).unwrap_or("3")},"domains":domains})
}

/** `PUT /json/new?url`: a tab under a lane of its own, described like a list entry. */
fn new_target(connect: &Connect, base: &str, url: &str) -> Result<Value, String> {
    let mut native = connect().map_err(|error| error.message)?;
    let params = if url.is_empty() {
        json!({})
    } else {
        json!({"url":url})
    };
    let opened = native
        .call("tab.open", params)
        .map_err(|error| error.to_string())?;
    let id = opened["tab_id"].as_str().unwrap_or_default();
    Ok(
        json!({"id":id,"type":"page","title":opened["page"]["title"].as_str().unwrap_or("New Tab"),"url":opened["page"]["url"].as_str().unwrap_or("about:blank"),"description":"","devtoolsFrontendUrl":"","webSocketDebuggerUrl":format!("{base}/devtools/page/{id}")}),
    )
}

/** `/json/close/<id>`: closes a tab, under the ownership rules tab.close applies. */
fn close_target(connect: &Connect, tab: &str) -> Result<(), String> {
    let mut native = connect().map_err(|error| error.message)?;
    let status = native
        .call("control.status", json!({"tab_id":tab}))
        .map_err(|error| error.to_string())?;
    native
        .call(
            "tab.close",
            json!({"tab_id":tab,"control_epoch":status["epoch"]}),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/** A JSON HTTP response, then close. */
fn respond(stream: &mut TcpStream, status: &str, body: &Value) -> std::io::Result<()> {
    let body = body.to_string();
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/** Complete the WebSocket handshake and run one CDP session on its own lane. */
fn upgrade(
    mut stream: TcpStream,
    key: &str,
    connect: &Connect,
    tab: Option<String>,
) -> std::io::Result<()> {
    let native = match connect() {
        Ok(native) => native,
        Err(error) => {
            return respond(
                &mut stream,
                "503 Service Unavailable",
                &json!({"error":error.message}),
            );
        }
    };
    write!(
        stream,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        websocket::accept_key(key)
    )?;
    let _ = stream.set_read_timeout(None);
    let mut session = Session::new(native, tab);
    while let Some(message) = websocket::read_message(&mut stream)? {
        for out in session.handle(&message) {
            websocket::write_text(&mut stream, &out.to_string())?;
        }
        if session.closing {
            break;
        }
    }
    Ok(())
}
