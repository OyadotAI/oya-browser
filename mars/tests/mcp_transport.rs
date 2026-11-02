/* Private pollable pipes exercise MCP transport failure, admission and exact-epoch cancellation. */
#![cfg(unix)]
use mars::{constants, protocol, runtime};
#[path = "../src/desktop/broker.rs"]
#[allow(dead_code)]
mod broker;
#[path = "../src/desktop/io.rs"]
#[allow(dead_code)]
mod io;
#[path = "../src/desktop/mcp_io.rs"]
#[allow(dead_code)]
mod mcp_io;
use protocol::{Request, Response};
use runtime::authority::{Authority, Owner, Permit, SharedAuthority};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    net::Shutdown,
    os::{fd::AsRawFd, unix::net::UnixStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/** Private descriptors stay owned until every transport thread has joined. */
struct Pipes {
    input: UnixStream,
    output: BufReader<UnixStream>,
    _server_input: UnixStream,
    _server_output: UnixStream,
    commands: Receiver<broker::Command>,
    admission: SyncSender<broker::Command>,
    responses: SyncSender<Response>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

/** Integration waits are bounded independently of whether transport cancellation works. */
const WAIT: Duration = Duration::from_secs(5);
/** Far beyond any OS pipe buffer, so an unread client blocks the writer, yet small enough that
building the frame costs nothing next to the write deadline. Sizing it from FRAME_BYTES made the
test measure serialization of the largest possible frame instead of the deadline. */
const BLOCKING_PAYLOAD_BYTES: usize = 1024 * 1024;
impl Pipes {
    /** Each test gets fresh pipes and authority without touching process stdin or secure profiles. */
    fn new() -> Self {
        Self::with_transport(true)
    }
    /** Both front doors use the same private descriptors and authority fixtures. */
    fn with_transport(mcp: bool) -> Self {
        let (input, server_input) = UnixStream::pair().unwrap();
        let (server_output, output) = UnixStream::pair().unwrap();
        output.set_read_timeout(Some(WAIT)).unwrap();
        let (admit, commands) = mpsc::sync_channel(constants::DESKTOP_QUEUE);
        let (responses, replies) = mpsc::sync_channel(constants::DESKTOP_QUEUE);
        let authority = Authority::shared(Owner::Agent);
        let cancel = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(true));
        let input_fd = io::Descriptor::new(server_input.as_raw_fd()).unwrap();
        let output_fd = io::Descriptor::new(server_output.as_raw_fd()).unwrap();
        let workers = if mcp {
            mcp_io::start_descriptors(
                input_fd,
                output_fd,
                admit.clone(),
                replies,
                authority.clone(),
                cancel.clone(),
                connected.clone(),
            )
        } else {
            io::start_descriptors(
                input_fd,
                output_fd,
                (admit.clone(), None),
                replies,
                authority.clone(),
                cancel.clone(),
                connected.clone(),
            )
        };
        Self {
            input,
            output: BufReader::new(output),
            _server_input: server_input,
            _server_output: server_output,
            commands,
            admission: admit,
            responses,
            authority,
            cancel,
            connected,
            workers,
        }
    }
    /** Protocol output is read as one whole frame rather than arbitrary byte fragments. */
    fn response(&mut self) -> Value {
        let mut frame = String::new();
        self.output.read_line(&mut frame).unwrap();
        serde_json::from_str(&frame).unwrap()
    }
    /** JSON serialization preserves literal escaped newlines inside a physical frame. */
    fn send(&mut self, value: Value) {
        self.input
            .write_all(&serde_json::to_vec(&value).unwrap())
            .unwrap();
        self.input.write_all(b"\n").unwrap();
    }
    /** Ready state is established using the public wire lifecycle, not direct session mutation. */
    fn initialize(&mut self) {
        self.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":constants::MCP_PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"transport-test","version":"1"}}}));
        assert_eq!(self.response()["id"], 0);
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }
    /** Tests admit only actual catalog tools and retain their external typed IDs. */
    fn tool(&mut self, id: u64, name: &str, arguments: Value) {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}}));
    }
    /** A missing cancellation transition fails quickly instead of hanging the suite. */
    fn wait_disconnected(&self) {
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline {
            let stopped = !self.connected.load(Ordering::Acquire)
                && self.cancel.load(Ordering::Acquire)
                && self.authority.lock().unwrap().tab_status(1).unwrap().0 == Owner::Human;
            if stopped {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(!self.connected.load(Ordering::Acquire));
        assert!(self.cancel.load(Ordering::Acquire));
        assert_eq!(
            self.authority.lock().unwrap().tab_status(1).unwrap().0,
            Owner::Human
        );
    }
    /** Observing a tool in the native queue synchronizes later cancellation checks. */
    fn command(&self) -> Request {
        self.admitted().0
    }
    /** Tests distinguish untrusted supplied epochs from the actual native admission permit. */
    fn admitted(&self) -> (Request, Permit) {
        let broker::Command::Agent(request, permit) = self.commands.recv_timeout(WAIT).unwrap()
        else {
            panic!("wrong command");
        };
        (request, permit)
    }
}
impl Drop for Pipes {
    /** Owned threads always relinquish borrowed descriptors before their streams close. */
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        for worker in self.workers.drain(..) {
            worker.join().unwrap();
        }
    }
}

/** Fragments execute only once the delimiter arrives and escaped strings remain one message. */
#[test]
fn fragmented_input_preserves_typed_ids_and_only_admits_complete_frames() {
    let mut pipes = Pipes::new();
    pipes
        .input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":\"a\\nb\",")
        .unwrap();
    assert!(pipes.commands.try_recv().is_err());
    pipes.input.write_all(b"\"method\":\"ping\"}\n").unwrap();
    assert_eq!(
        pipes.response(),
        json!({"jsonrpc":"2.0","id":"a\nb","result":{}})
    );
}

/** EOF never interprets an unterminated partial JSON request. */
#[test]
fn partial_frame_eof_cancels_without_admitting_the_request() {
    let mut pipes = Pipes::new();
    pipes
        .input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}")
        .unwrap();
    pipes.input.shutdown(Shutdown::Write).unwrap();
    pipes.wait_disconnected();
    assert!(pipes.commands.try_recv().is_err());
}

/** Idle EOF is enough to revoke all tab grants and request window cleanup. */
#[test]
fn idle_eof_immediately_revokes_agent_ownership_and_cancels() {
    let pipes = Pipes::new();
    pipes.input.shutdown(Shutdown::Write).unwrap();
    pipes.wait_disconnected();
}

/** Native NDJSON retains its prior keep-window-on-EOF cancellation policy. */
#[test]
fn native_ndjson_eof_revokes_ownership_without_cancelling_the_window_runtime() {
    let pipes = Pipes::with_transport(false);
    pipes.input.shutdown(Shutdown::Write).unwrap();
    let deadline = Instant::now() + WAIT;
    while pipes.connected.load(Ordering::Acquire) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(!pipes.connected.load(Ordering::Acquire));
    assert!(!pipes.cancel.load(Ordering::Acquire));
    assert_eq!(
        pipes.authority.lock().unwrap().tab_status(1).unwrap().0,
        Owner::Human
    );
}

/** Input accumulation rejects the byte ceiling before trying JSON parsing or native work. */
#[test]
fn oversized_unterminated_input_retires_the_connection() {
    let mut pipes = Pipes::new();
    // The reader retires the connection as soon as the ceiling is crossed, so the tail of this
    // write may meet a closed pipe; that is the retirement, not a failure of the test.
    let _ = pipes
        .input
        .write_all(&vec![b' '; constants::MCP_FRAME_BYTES]);
    pipes.wait_disconnected();
    assert!(pipes.commands.try_recv().is_err());
}

/** A closed stdout peer cannot retain an initialized session or mutation grants. */
#[test]
fn broken_output_retires_the_connection() {
    let mut pipes = Pipes::new();
    pipes.output.get_ref().shutdown(Shutdown::Read).unwrap();
    pipes.send(json!({"jsonrpc":"2.0","id":1,"method":"ping"}));
    pipes.wait_disconnected();
}

/** A blocked complete-frame write expires even if some bytes were already accepted. */
#[test]
fn output_backpressure_uses_an_absolute_complete_frame_deadline() {
    let mut pipes = Pipes::new();
    pipes.initialize();
    pipes.tool(1, "runtime_capabilities", json!({}));
    let request = pipes.command();
    pipes
        .responses
        .send(Response::new(
            Some(request.id),
            Ok(json!({"data":"x".repeat(BLOCKING_PAYLOAD_BYTES)})),
        ))
        .unwrap();
    pipes.wait_disconnected();
}

/** A full native queue returns a correlated tool error and leaves protocol use available. */
#[test]
fn saturated_native_admission_returns_busy_without_retiring_the_session() {
    let mut pipes = Pipes::new();
    pipes.initialize();
    for index in 0..constants::DESKTOP_QUEUE {
        assert!(
            pipes
                .admission
                .try_send(broker::Command::Agent(
                    Request::new(
                        &format!("occupied-{index}"),
                        "runtime.capabilities",
                        json!({}),
                    ),
                    Permit {
                        owner: Owner::Agent,
                        epoch: 1,
                        tab_id: None,
                    },
                ))
                .is_ok()
        );
    }
    for id in 1..=(constants::MCP_PENDING_REQUESTS * 2) as u64 {
        pipes.tool(id, "runtime_capabilities", json!({}));
        let response = pipes.response();
        assert_eq!(response["id"], id);
        assert_eq!(response["result"]["isError"], true);
        assert_eq!(
            response["result"]["structuredContent"]["error"]["code"],
            "busy"
        );
    }
    assert!(pipes.connected.load(Ordering::Acquire));
    assert_eq!(
        pipes.authority.lock().unwrap().tab_status(1),
        Some((Owner::Agent, 1))
    );
}

/** Exact cancellation invalidates queued permits while suppressing only its eventual reply. */
#[test]
fn cancelled_mutation_revokes_only_its_matching_tab_epoch() {
    let mut pipes = Pipes::new();
    pipes.initialize();
    pipes.tool(
        1,
        "page_navigate",
        json!({"tab_id":"tab-1","control_epoch":1,"url":"https://example.com/"}),
    );
    let request = pipes.command();
    pipes
        .send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}));
    pipes.send(json!({"jsonrpc":"2.0","id":2,"method":"ping"}));
    assert_eq!(pipes.response()["id"], 2);
    assert_eq!(
        pipes.authority.lock().unwrap().tab_status(1),
        Some((Owner::Human, 2))
    );
    pipes
        .responses
        .send(Response::new(Some(request.id), Ok(json!({}))))
        .unwrap();
    assert!(pipes.connected.load(Ordering::Acquire));
}

/** A guessed future client epoch cannot become authority for cancellation after a fresh grant. */
#[test]
fn guessed_future_epoch_cannot_revoke_a_fresh_human_grant() {
    let mut pipes = Pipes::new();
    pipes.initialize();
    pipes.tool(
        1,
        "page_navigate",
        json!({"tab_id":"tab-1","control_epoch":3,"url":"https://example.com/"}),
    );
    let (_, captured) = pipes.admitted();
    assert_eq!(captured.epoch, 1);
    assert_eq!(captured.tab_id, Some(1));
    {
        let mut state = pipes.authority.lock().unwrap();
        state.change_tab(1, Owner::Human).unwrap();
        state.change_tab(1, Owner::Agent).unwrap();
    }
    pipes
        .send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}));
    pipes.send(json!({"jsonrpc":"2.0","id":2,"method":"ping"}));
    assert_eq!(pipes.response()["id"], 2);
    assert_eq!(
        pipes.authority.lock().unwrap().tab_status(1),
        Some((Owner::Agent, 3))
    );
}

/** Completed admissions free their permits before the next bounded batch enters native work. */
#[test]
fn successful_native_completions_release_captured_admission_records() {
    let mut pipes = Pipes::new();
    pipes.initialize();
    for id in 1..=(constants::MCP_PENDING_REQUESTS * 2) as u64 {
        pipes.tool(id, "runtime_capabilities", json!({}));
        let request = pipes.command();
        pipes
            .responses
            .send(Response::new(Some(request.id), Ok(json!({"ready":true}))))
            .unwrap();
        assert_eq!(pipes.response()["id"], id);
    }
    assert!(pipes.connected.load(Ordering::Acquire));
}

/** Read cancellation suppresses its response without changing tab authority. */
#[test]
fn cancelling_a_read_does_not_revoke_agent_ownership() {
    let mut pipes = Pipes::new();
    pipes.initialize();
    pipes.tool(1, "page_inspect", json!({"tab_id":"tab-1"}));
    let _ = pipes.command();
    pipes
        .send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}));
    pipes.send(json!({"jsonrpc":"2.0","id":2,"method":"ping"}));
    assert_eq!(pipes.response()["id"], 2);
    assert_eq!(
        pipes.authority.lock().unwrap().tab_status(1),
        Some((Owner::Agent, 1))
    );
}

/** A late cancellation never revokes a newer grant created by the person. */
#[test]
fn cancelled_old_epoch_preserves_a_fresh_human_grant() {
    let mut pipes = Pipes::new();
    pipes.initialize();
    pipes.tool(
        1,
        "page_navigate",
        json!({"tab_id":"tab-1","control_epoch":1,"url":"https://example.com/"}),
    );
    let _ = pipes.command();
    {
        let mut state = pipes.authority.lock().unwrap();
        state.change_tab(1, Owner::Human).unwrap();
        state.change_tab(1, Owner::Agent).unwrap();
    }
    pipes
        .send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}));
    pipes.send(json!({"jsonrpc":"2.0","id":2,"method":"ping"}));
    assert_eq!(pipes.response()["id"], 2);
    assert_eq!(
        pipes.authority.lock().unwrap().tab_status(1),
        Some((Owner::Agent, 3))
    );
}

/** Launch selection fails before any window or authority exists. */
#[test]
fn mcp_and_native_launch_flags_are_exclusive_and_default_to_human_control() {
    let parse = |flags: &[&str]| {
        mars::desktop::Options::parse(&flags.iter().map(|flag| (*flag).into()).collect::<Vec<_>>())
    };
    assert!(parse(&["--mcp", "--agent"]).is_err());
    assert!(parse(&["--agent-control"]).is_err());
    let options = parse(&["--mcp"]).unwrap();
    assert!(options.mcp && !options.agent && !options.agent_control);
    assert!(parse(&["--mcp", "--agent-control"]).unwrap().agent_control);
}
