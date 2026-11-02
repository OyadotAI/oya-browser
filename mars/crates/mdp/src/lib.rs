/**
 * The Mars CDP front door. External Chrome DevTools Protocol clients (Puppeteer, Playwright's
 * connectOverCDP, raw CDP scripts) connect over a loopback WebSocket; every supported method is
 * translated into native Mars operations sent through the browser's attach endpoint, so each CDP
 * connection is one agent lane under the same ownership, epoch and redaction rules as MCP.
 * Nothing here speaks CDP to an engine: Mars has no debugger, and unsupported methods return an
 * explicit error instead of a fallback. `catalog` reads the pinned protocol definitions and
 * `matrix` says, for every command, where Mars stands (docs/cdp.md).
 */
pub mod catalog;
pub mod matrix;
pub mod server;
pub mod session;
pub mod websocket;
use mars::{
    constants::VERSION,
    mcp::attach::{Endpoint, Protocol},
    protocol::EngineResult,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

/** One native call: the operation's result, or the engine error object. */
pub trait Native {
    /// Run one native operation for this connection's lane.
    fn call(&mut self, op: &str, params: Value) -> Result<Value, Value>;
}

/** A native NDJSON connection to the running browser's attach endpoint: one lane. */
pub struct Attached {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next: u64,
    /// The lane the browser assigned to this connection.
    pub lane: String,
}

impl Attached {
    /** Attach as a native client. */
    pub fn connect(endpoint: &Endpoint) -> EngineResult<Self> {
        let (stream, lane) = endpoint.connect(Protocol::Native)?;
        let _ = stream.set_read_timeout(None);
        let reader = BufReader::new(stream.try_clone().map_err(|_| closed_error())?);
        Ok(Self {
            stream,
            reader,
            next: 0,
            lane,
        })
    }
}

impl Native for Attached {
    /** Requests are sequential, so the next response with our ID is the answer. */
    fn call(&mut self, op: &str, params: Value) -> Result<Value, Value> {
        self.next += 1;
        let id = format!("cdp-{}", self.next);
        let request = json!({"version":VERSION,"id":id,"op":op,"params":params});
        if self
            .stream
            .write_all(format!("{request}\n").as_bytes())
            .is_err()
        {
            return Err(closed());
        }
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).unwrap_or(0) == 0 {
                return Err(closed());
            }
            let Ok(response) = serde_json::from_str::<Value>(&line) else {
                return Err(closed());
            };
            if response["id"] != id.as_str() {
                continue;
            }
            return match response.get("result") {
                Some(result) => Ok(result.clone()),
                None => Err(response.get("error").cloned().unwrap_or_else(closed)),
            };
        }
    }
}

/** The attach connection ended. */
fn closed() -> Value {
    json!({"code":"browser_not_running","message":"The Mars browser connection closed."})
}

/** The same, as an engine error. */
fn closed_error() -> mars::protocol::EngineError {
    mars::protocol::EngineError::new(
        "browser_not_running",
        "The Mars browser connection closed.",
        "Start the browser and reconnect.",
    )
}
