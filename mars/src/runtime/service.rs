/** Typed private operations keep engine data separate from native agent authority. */
use crate::document::{self, Document, DomChange};
use crate::protocol::{
    EngineError, EngineResult, RequestHead, Response, TypedRequest, decode, encode, encode_reply,
    parameters, read_frame,
};
use crate::{network, v9};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Cursor, Write};
use std::sync::mpsc::{Receiver, SyncSender};

/** Private dispatch never shares the public agent operation table. */
type Handler = fn(Value) -> EngineResult<Value>;
/** Operations whose parameters or results are whole documents, sources or responses: they read
their exact parameter type from the frame and write their result straight into the reply. */
type TypedHandler = fn(&[u8], &str) -> EngineResult<Vec<u8>>;

/// Ownership-pipe EOF and failed writes terminate services without background descendants.
/// The loop runs on a SERVICE_STACK_BYTES thread: deep documents recurse further than the main
/// thread's stack allows.
pub fn run_service(name: &str) -> EngineResult<()> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name(name.into())
            .stack_size(crate::constants::SERVICE_STACK_BYTES)
            .spawn_scoped(scope, || serve(name))
            .map_err(|_| EngineError::unsupported("The service could not start its thread."))?
            .join()
            .unwrap_or_else(|_| {
                Err(EngineError::invalid(
                    "The service stopped on an internal error.",
                ))
            })
    })
}

/** Reader adapter turning channel message frames into a byte stream; typed jobs that arrive
between frames run here, on the engine's thread, and answer through their own channel. */
struct ChannelReader {
    receiver: Receiver<super::supervisor::Inbound>,
    /** Set when the owning service drops: the thread exits though senders remain. */
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    current: Cursor<Vec<u8>>,
}

impl ChannelReader {
    /** Creates a new reader connected to the request receiver. */
    fn new(
        receiver: Receiver<super::supervisor::Inbound>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            receiver,
            stop,
            current: Cursor::new(Vec::new()),
        }
    }

    /** Refills the internal buffer with the next frame from the channel. */
    fn refill(&mut self) -> io::Result<bool> {
        use std::sync::mpsc::RecvTimeoutError;
        loop {
            if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
                return Ok(false);
            }
            match self.receiver.recv_timeout(crate::constants::RPC_POLL) {
                Ok(super::supervisor::Inbound::Frame(frame)) => {
                    self.current = Cursor::new(frame);
                    return Ok(true);
                }
                Ok(super::supervisor::Inbound::Job(job)) => run_job(job),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(false),
            }
        }
    }
}

/** A job that panics drops its answer channel, so its caller sees a failed service call
instead of this engine thread dying with it. */
fn run_job(job: super::supervisor::Job) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
}

impl io::Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available = self
            .current
            .get_ref()
            .len()
            .saturating_sub(self.current.position() as usize);
        if available == 0 && !self.refill()? {
            return Ok(0);
        }
        self.current.read(buf)
    }
}

impl io::BufRead for ChannelReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let available = self
            .current
            .get_ref()
            .len()
            .saturating_sub(self.current.position() as usize);
        if available == 0 && !self.refill()? {
            return Ok(&[]);
        }
        self.current.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.current.consume(amt);
    }
}

/** Writer adapter forwarding buffered reply frames into a sync channel. */
struct ChannelWriter {
    sender: SyncSender<EngineResult<Vec<u8>>>,
    buffer: Vec<u8>,
}

impl ChannelWriter {
    /** Creates a new writer connected to the reply channel. */
    fn new(sender: SyncSender<EngineResult<Vec<u8>>>) -> Self {
        Self {
            sender,
            buffer: Vec::new(),
        }
    }
}

impl io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let frame = std::mem::take(&mut self.buffer);
        self.sender
            .send(Ok(frame))
            .map_err(|err| io::Error::new(io::ErrorKind::BrokenPipe, err))
    }
}

/** Reads framed requests until the owner's pipe closes, answering each in order. */
fn serve(name: &str) -> EngineResult<()> {
    let _owner = super::lifetime::OwnerWatch::start()?;
    serve_stream(name, BufReader::new(io::stdin()), io::stdout())
}

/** Run an in-process engine worker servicing requests over memory channels. */
pub(super) fn serve_in_process(
    name: &str,
    requests: Receiver<super::supervisor::Inbound>,
    replies: SyncSender<EngineResult<Vec<u8>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> EngineResult<()> {
    let reader = ChannelReader::new(requests, stop);
    let writer = ChannelWriter::new(replies);
    serve_stream(name, reader, writer)
}

/** Dispatches a single network request token preparation. */
fn prepare_network_token(
    frame: &[u8],
    id: &str,
    prepared: &mut Option<(u64, network::Prepared)>,
    sequence: &mut u64,
) -> EngineResult<Vec<u8>> {
    prepared.take();
    *sequence = sequence.saturating_add(1);
    let current_seq = *sequence;
    value_request(frame)
        .and_then(prepare)
        .map(|connection| {
            *prepared = Some((current_seq, connection));
            json!({"token": current_seq})
        })
        .and_then(|result| encode_reply(id, &result))
}

/** Dispatches a network transmit operation against an active prepared connection. */
fn transmit_network_token(
    frame: &[u8],
    id: &str,
    prepared: &mut Option<(u64, network::Prepared)>,
) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Transmit| {
        prepared
            .take()
            .filter(|(token, _)| *token == input.token)
            .ok_or_else(|| EngineError::invalid("Prepared request token expired or was consumed."))
            .and_then(|(_, connection)| connection.transmit())
    })
}

/** Dispatches one request frame to the appropriate engine service handler. */
fn dispatch_service_frame(
    name: &str,
    head: &RequestHead,
    frame: &[u8],
    prepared: &mut Option<(u64, network::Prepared)>,
    sequence: &mut u64,
    input: &mut (dyn BufRead + Send),
    output: &mut (dyn Write + Send),
) -> EngineResult<Vec<u8>> {
    if name == "network" && head.op == "network.prepare" {
        prepare_network_token(frame, &head.id, prepared, sequence)
    } else if name == "network" && head.op == "network.transmit" {
        transmit_network_token(frame, &head.id, prepared)
    } else if name == "v9" && head.op == "v9.worker" {
        let imports = super::worker_io::Pipe::new(&head.id, input, output);
        typed(frame, &head.id, |call| run_page_host(call, Some(&imports)))
    } else {
        dispatch(name, &head.op, &head.id, frame)
    }
}

/** Writes a framed reply (or encoded error response) to the output stream. */
fn write_service_reply(
    output: &mut (dyn Write + Send),
    head_id: Option<String>,
    reply: EngineResult<Vec<u8>>,
) -> EngineResult<()> {
    let bytes = match reply {
        Ok(bytes) => bytes,
        Err(error) => encode(&Response::new(head_id, Err(error)))?,
    };
    output.write_all(&bytes).map_err(closed)?;
    output.flush().map_err(closed)
}

/** Reads framed requests until stream closes, answering each in order. */
pub(super) fn serve_stream(
    name: &str,
    mut input: impl BufRead + Send,
    mut output: impl Write + Send,
) -> EngineResult<()> {
    if !["network", "document", "v9"].contains(&name) {
        return Err(EngineError::invalid("Unknown engine service."));
    }
    let mut prepared: Option<(u64, network::Prepared)> = None;
    let mut sequence = 0_u64;
    while let Some(frame) = read_frame(&mut input)? {
        let head: RequestHead = decode(&frame)?;
        head.validate()?;
        let head_id = head.id.clone();
        let reply = dispatch_service_frame(
            name,
            &head,
            &frame,
            &mut prepared,
            &mut sequence,
            &mut input,
            &mut output,
        );
        write_service_reply(&mut output, Some(head_id), reply)?;
    }
    Ok(())
}

/** The same failure `write_message` reports when the owner's pipe is gone. */
fn closed(_: io::Error) -> EngineError {
    EngineError::invalid("The transport closed or could not be read or written.")
}

/** A request's parameters as a `Value`, for the small operations that take one. */
fn value_request(frame: &[u8]) -> EngineResult<Value> {
    decode::<TypedRequest<Value>>(frame).map(|request| request.params)
}

/** Decodes `P` straight from the frame, runs the operation and encodes its reply. */
fn typed<P: DeserializeOwned, R: Serialize>(
    frame: &[u8],
    id: &str,
    run: impl FnOnce(P) -> EngineResult<R>,
) -> EngineResult<Vec<u8>> {
    let request: TypedRequest<P> = serde_json::from_slice(frame)
        .map_err(|_| EngineError::invalid("Parameters do not match this operation's schema."))?;
    encode_reply(id, &run(request.params)?)
}

/** A correlated readiness response proves the child speaks this exact protocol. */
fn dispatch(name: &str, op: &str, id: &str, frame: &[u8]) -> EngineResult<Vec<u8>> {
    let typed_handlers: &[(&str, &str, TypedHandler)] = &[
        ("network", "network.fetch_batch", fetch_batch),
        ("document", "document.parse", parse),
        ("document", "document.apply", apply),
        ("document", "document.render", render),
        ("document", "document.styles", styles),
        ("document", "document.frame_styles", frame_styles),
        ("document", "document.frame_render", frame_render),
        ("document", "document.layout", layout),
        ("document", "document.value", value),
        ("v9", "v9.page", page),
        ("v9", "v9.devtools", devtools),
        ("v9", "v9.execute", execute),
        ("v9", "v9.load", load),
        ("v9", "v9.event", event),
        ("v9", "v9.document_load", document_load),
        ("v9", "v9.document_click", document_click),
    ];
    if let Some((_, _, handler)) = typed_handlers
        .iter()
        .find(|(service, operation, _)| *service == name && *operation == op)
    {
        return handler(frame, id);
    }
    let params = value_request(frame)?;
    encode_reply(id, &dispatch_value(name, op, params)?)
}

/** The operations that still exchange small `Value` parameters. */
fn dispatch_value(name: &str, op: &str, params: Value) -> EngineResult<Value> {
    if op == "service.ready" {
        let _: Empty = parameters(params)?;
        return Ok(json!({"service":name,"ready":true}));
    }
    let handlers: &[(&str, &str, Handler)] = &[
        ("network", "network.fetch", fetch),
        ("network", "network.web_fetch", web_fetch),
        ("network", "network.cache_clear", cache_clear),
        ("network", "network.cache_forget", cache_forget),
    ];
    let handler = handlers
        .iter()
        .find(|(service, operation, _)| *service == name && *operation == op)
        .map(|(_, _, handler)| handler)
        .ok_or_else(|| EngineError::invalid("Unknown private service operation."))?;
    handler(params)
}

/** Empty schemas reject unrecognized fields. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
/** URLs whose cached copies another network process made stale. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Forget {
    /** Absolute URLs without fragments, as Relay keys them. */
    urls: Vec<String>,
}
/** Only the network owner receives an outbound URL. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fetch {
    /** Literal loopback URL. */
    url: String,
}
/** Fixture grants originate at startup, never at a public page operation. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WebFetch {
    /** Normalized outbound URL. */
    url: String,
    /** Exact owned origins. */
    fixture_origins: Vec<String>,
}
/** Preparation has no authority to transmit HTTP application bytes. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    request: network::WebRequest,
    fixture_origins: Vec<String>,
    remaining_ms: u64,
}
/** Independent subresource GETs the coordinator already shaped hop by hop. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FetchBatch {
    /** At most FETCH_BATCH requests, each validated by Relay exactly as a single fetch is. */
    requests: Vec<network::WebRequest>,
    /** Exact owned origins. */
    fixture_origins: Vec<String>,
    /** The navigation's remaining deadline, shared by every request in the batch. */
    remaining_ms: u64,
}
/** One request's outcome; a failure stays this request's, never the batch's. */
#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum BatchOutcome {
    /** The response, a redirect included, for the coordinator to follow under its own policy. */
    Response(network::WebResponse),
    /** Why this request failed. */
    Error(EngineError),
}
/** A one-use private token is not reachable through the agent front door. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transmit {
    token: u64,
}
/** Stateful preparation remains isolated to the one owned network process. */
fn prepare(params: Value) -> EngineResult<network::Prepared> {
    let input: Prepare = parameters(params)?;
    network::Prepared::connect(input.request, &input.fixture_origins, input.remaining_ms)
}
/** Source carries no coordinator capabilities. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parse {
    /** Bounded HTML. */
    source: String,
    /** Canonical encoding Relay decoded with, or none when the transport declared no charset. */
    #[serde(default)]
    charset: Option<String>,
    /** The response is an XML document (XHTML or SVG), parsed as XML rather than HTML. */
    #[serde(default)]
    xml: bool,
}
/** Candidate mutation is atomic over explicit input data. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Apply {
    /** Candidate snapshot. */
    document: Document,
    /** Inert ordered proposals, replayed in script order. */
    changes: Vec<DomChange>,
    /** V9's extended log, replayed after `changes` in the same atomic candidate. */
    #[serde(default)]
    script_changes: Vec<document::ScriptChange>,
}
/** Layout never resolves a hidden page. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    /** Explicit snapshot. */
    document: Document,
}
/** One point viewport feeds both painting and inspection. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Render {
    /** Exact snapshot. */
    document: Document,
    /** Logical-point width. */
    #[serde(default)]
    width: Option<f64>,
    /** Same-build desktop callers supply all host presentation facts together. */
    #[serde(default)]
    context: Option<document::RenderContext>,
    /** Scroll container offsets by node identity; Eye clamps them. */
    #[serde(default)]
    scroll: document::ScrollOffsets,
}
/** CSS sources have already passed resource acquisition and byte limits. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Styles {
    document: Document,
    sheets: Vec<String>,
    /** Legacy callers retain deterministic default presentation. */
    #[serde(default)]
    context: Option<document::RenderContext>,
}
/** Style visibility and paint metadata come from the same owned cascade. */
fn styles(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Styles| {
        document::compute_styles_with_context(
            &input.document,
            &input.sheets,
            input.context.unwrap_or_default(),
        )
    })
}
/** A frame's document styled for its content box: the same cascade under frame bounds. */
fn frame_styles(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Styles| {
        let context = input
            .context
            .ok_or_else(|| EngineError::invalid("A frame needs its viewport."))?;
        document::compute_frame_styles(&input.document, &input.sheets, context)
    })
}
/** A frame's document laid out in its content box, its scroll containers at `scroll`. */
fn frame_render(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Render| {
        let context = input
            .context
            .ok_or_else(|| EngineError::invalid("A frame needs its viewport."))?;
        document::render_frame_scrolled(&input.document, context, &input.scroll)
    })
}
/** Input editing does not reinterpret its value as source. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetValue {
    /** Exact snapshot. */
    document: Document,
    /** Native local identity. */
    local_id: usize,
    /** Literal value. */
    value: String,
}
/** Legacy V9 source-only execution. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Execute {
    /** Bounded source. */
    source: String,
}
/** Page loading receives only bounded input values. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Load {
    /** Ordered page programs share one VM candidate and execution budget. */
    sources: Vec<String>,
    /** Explicit DOM reads. */
    inputs: Vec<v9::InputValue>,
    /** Candidate Storage maps supplied by the coordinator for this origin and tab. */
    #[serde(default)]
    storage: v9::StorageState,
}
/** Function execution receives committed string globals, not host objects. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    /** Original ordered page programs. */
    sources: Vec<String>,
    /** One supported named call. */
    handler: String,
    /** Committed globals. */
    state: v9::VmState,
    /** Current DOM input values. */
    inputs: Vec<v9::InputValue>,
    /** Candidate Storage maps supplied by the coordinator for this origin and tab. */
    #[serde(default)]
    storage: v9::StorageState,
}

/** Projected loading gives page code only bounded DOM data and origin-local storage. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentLoad {
    /** Ordered source programs share one candidate transaction. */
    sources: Vec<String>,
    /** Current real document projection has no native authority. */
    document: document::ScriptDocument,
    /** Coordinator-selected origin and tab candidate values. */
    #[serde(default)]
    storage: v9::StorageState,
    /** Coordinator-read preferred languages for navigator.languages. */
    #[serde(default)]
    languages: Vec<String>,
}
/** A click identifies a projected local node rather than evaluating caller-provided source. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentClick {
    /** Original page programs validate retained callable code. */
    sources: Vec<String>,
    /** Current projected button identity. */
    target: usize,
    /** Committed VM graph for this page. */
    state: v9::VmState,
    /** Fresh projection contains current field values and node membership. */
    document: document::ScriptDocument,
    /** Candidate storage remains atomic with VM and DOM changes. */
    #[serde(default)]
    storage: v9::StorageState,
    /** Coordinator-read preferred languages for navigator.languages. */
    #[serde(default)]
    languages: Vec<String>,
}

/** One page lifecycle request: the projection, its sources and the facts the page sees. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageCall {
    /** Runtime-owned leases; required for resident execution, never inferred from page source. */
    #[serde(default)]
    resident_contexts: Option<Vec<String>>,
    /** The page's classic scripts in execution order (failed ones keep their text, since their
    declarations outlive them); empty when `sources_digest` names sources this process holds. */
    sources: Vec<String>,
    /** SHA-256 of the sources (`sources_digest`), so a page's megabytes of script cross the pipe
    once instead of on every lifecycle step. */
    #[serde(default)]
    sources_digest: Option<String>,
    /** Current projection; it carries no native authority or secret values. */
    document: document::ScriptDocument,
    /** Coordinator-selected origin and tab candidate values. */
    #[serde(default)]
    storage: v9::StorageState,
    /** Coordinator-read preferred languages for navigator.languages. */
    #[serde(default)]
    languages: Vec<String>,
    /** URL, viewport, clock, cookie string (never HttpOnly) and node text. */
    environment: v9::PageEnvironment,
    /** The page's module scripts, import maps and fetched modules; none for a classic page. */
    #[serde(default)]
    modules: v9::ModuleInput,
    /** SHA-256 of `modules` (`modules_digest`); `modules` is then empty when this process
    already holds them, so a page's fetched modules (2.75 MB on YouTube) cross the pipe once. */
    #[serde(default)]
    modules_digest: Option<String>,
    /** Committed engine state; absent only for the first load. */
    #[serde(default)]
    state: Option<v9::VmState>,
    /** Keep the resulting state in this process and reply with its token and summary. */
    #[serde(default)]
    resident: bool,
    /** The resident state this step starts from, instead of the state itself. */
    #[serde(default)]
    base: Option<u64>,
    /** The page's committed state, kept beside the step's base so an aborted step can return
    to it. */
    #[serde(default)]
    keep: Option<u64>,
    /** What to run against the state. */
    action: PageAction,
}
/** The V9 page entry points, one per lifecycle step. */
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum PageAction {
    /** Run the page's scripts, each rolled back alone on failure. */
    Load,
    /** Fire DOMContentLoaded or load. */
    Event { event: v9::PageEvent },
    /** A trusted click on a projected node. */
    Click { target: usize },
    /** A person typed into a field the document already updated; None hides a secret value. */
    Input {
        target: usize,
        #[serde(default)]
        value: Option<String>,
    },
    /** Typed strokes into one field, each delivered as a person's keyboard would; `value` is the
    field's text, absent for a secret field whose text never enters the engine. */
    Typed {
        target: usize,
        strokes: Vec<v9::Stroke>,
        #[serde(default)]
        value: Option<String>,
    },
    /** A native key dispatches trusted events and only its typed implemented default action. */
    Keyboard {
        key: v9::KeyboardKey,
        #[serde(default)]
        target: Option<usize>,
    },
    /** Advance the virtual clock and run due timers. */
    Timers { now_ms: f64, budget: usize },
    /** Sample layout and deliver intersection records without firing unrelated author timers. */
    Observe,
    /** The viewport changed while the page was idle: media query listeners hear it. */
    Media,
    /** Fresh layout resumes a depth-gated resize cycle; complete ends it at the host deadline. */
    Resize { complete: bool },
    /** Hand back responses to the page's requests. */
    Resume { responses: Vec<v9::HostResponse> },
    /** Commit real image decoding outcomes as asynchronous image lifecycle tasks. */
    Images { responses: Vec<v9::ImageResponse> },
    /** Continue module graphs with the modules fetched since the last step. */
    Modules,
}

/** A DevTools request (Runtime.evaluate and friends) over the same projection and sources as a
lifecycle step; the result rides beside the Execution the coordinator commits. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DevtoolsCall {
    /** The live context snapshot shared with ordinary lifecycle admission. */
    #[serde(default)]
    resident_contexts: Option<Vec<String>>,
    sources: Vec<String>,
    #[serde(default)]
    sources_digest: Option<String>,
    document: document::ScriptDocument,
    #[serde(default)]
    storage: v9::StorageState,
    #[serde(default)]
    languages: Vec<String>,
    environment: v9::PageEnvironment,
    #[serde(default)]
    modules: v9::ModuleInput,
    #[serde(default)]
    modules_digest: Option<String>,
    #[serde(default)]
    state: Option<v9::VmState>,
    /** Keep the resulting state in this process and reply with its token and summary. */
    #[serde(default)]
    resident: bool,
    /** The resident state this step starts from, instead of the state itself. */
    #[serde(default)]
    base: Option<u64>,
    /** The page's committed state, kept beside the step's base so an aborted step can return
    to it. */
    #[serde(default)]
    keep: Option<u64>,
    request: v9::devtools::Request,
}

/** One DevTools request in this process, under V9's own bounds and deadline. */
fn devtools(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, run_devtools)
}
/** The request over its decoded call; sources are resident exactly as for `run_page`. */
fn run_devtools(mut call: DevtoolsCall) -> EngineResult<v9::devtools::Outcome> {
    if call.resident {
        admit_resident(
            &call.document.identity,
            call.resident_contexts.as_deref(),
            call.base,
            call.keep,
        )?;
    }
    let sources = resident_sources(
        std::mem::take(&mut call.sources),
        call.sources_digest.take(),
    )?;
    let modules = resident_modules(
        std::mem::take(&mut call.modules),
        call.modules_digest.take(),
    )?;
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_eye::platform::NativeCanvas,
        languages: &call.languages,
        fragments: &document::DomFragments,
    };
    let input = v9::PageInput {
        sources: &sources.texts,
        source_keys: &sources.keys,
        document: call.document,
        storage: call.storage,
        host: &host,
        environment: &call.environment,
        modules: &modules,
    };
    let state = match call.state {
        Some(state) => Some(state),
        None => call.base.map(resident_state).transpose()?,
    };
    let mut outcome = v9::devtools::run(input, state, call.request)?;
    if call.resident {
        keep_resident(&mut outcome.execution, call.base, call.keep);
    }
    Ok(outcome)
}

/** Every lifecycle step runs in this process, under V9's own bounds and deadline. */
fn page(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, run_page)
}
/** One V9 page step over its decoded call. */
fn run_page(call: PageCall) -> EngineResult<v9::Execution> {
    run_page_host(call, None)
}

/** Only the explicit worker operation receives the temporary synchronous source capability. */
fn run_page_host(
    mut call: PageCall,
    imports: Option<&dyn v9::ImportScriptsHost>,
) -> EngineResult<v9::Execution> {
    if call.environment.worker_name.is_some() != imports.is_some() {
        return Err(EngineError::invalid("Invalid worker execution capability."));
    }
    if call.resident {
        admit_resident(
            &call.document.identity,
            call.resident_contexts.as_deref(),
            call.base,
            call.keep,
        )?;
    }
    let sources = resident_sources(
        std::mem::take(&mut call.sources),
        call.sources_digest.take(),
    )?;
    let modules = resident_modules(
        std::mem::take(&mut call.modules),
        call.modules_digest.take(),
    )?;
    let host = v9::DocumentHost {
        imports,
        canvas: &mars_eye::platform::NativeCanvas,
        languages: &call.languages,
        fragments: &document::DomFragments,
    };
    let input = v9::PageInput {
        sources: &sources.texts,
        source_keys: &sources.keys,
        document: call.document,
        storage: call.storage,
        host: &host,
        environment: &call.environment,
        modules: &modules,
    };
    let state = || match (&call.state, call.base) {
        (Some(state), _) => Ok(state.clone()),
        (None, Some(token)) => resident_state(token),
        (None, None) => Err(EngineError::invalid(
            "This V9 page step needs committed state.",
        )),
    };
    let mut execution = run_action(call.action, input, state, imports.is_some())?;
    if call.resident {
        keep_resident(&mut execution, call.base, call.keep);
    }
    Ok(execution)
}

/** Run one lifecycle action against the input, taking committed state only when it needs it. */
fn run_action(
    action: PageAction,
    input: v9::PageInput<'_>,
    state: impl FnOnce() -> EngineResult<v9::VmState>,
    worker: bool,
) -> EngineResult<v9::Execution> {
    match action {
        PageAction::Load if worker => v9::load_worker(input),
        PageAction::Load => v9::load_page(input),
        PageAction::Event { event } => v9::page_event(input, state()?, event),
        PageAction::Click { target } => v9::click_page(input, state()?, target),
        PageAction::Input { target, value } => v9::input_page(input, state()?, target, value),
        PageAction::Typed {
            target,
            strokes,
            value,
        } => v9::typed_page(input, state()?, target, strokes, value),
        PageAction::Keyboard { key, target } => v9::keyboard_page(input, state()?, key, target),
        PageAction::Timers { now_ms, budget } => v9::run_timers(input, state()?, now_ms, budget),
        PageAction::Observe => v9::observe_page(input, state()?),
        PageAction::Media => v9::report_media(input, state()?),
        PageAction::Resize { complete } => v9::continue_resize_observers(input, state()?, complete),
        PageAction::Resume { responses } => v9::resume_requests(input, state()?, responses),
        PageAction::Images { responses } => v9::resume_images(input, state()?, responses),
        PageAction::Modules if worker => v9::resume_worker_modules(input, state()?),
        PageAction::Modules => v9::load_modules(input, state()?),
    }
}

/**
 * A page step handed over in process as owned values: the scripts and modules are shared with
 * the coordinator rather than copied, encoded or hashed, and the state lives here under its
 * token exactly as a framed resident step's does.
 */
pub(super) struct TypedPage {
    /** The page's classic scripts with their code-cache keys, computed once per script list. */
    pub sources: std::sync::Arc<ResidentSources>,
    /** Module scripts, import maps and fetched modules. */
    pub modules: std::sync::Arc<v9::ModuleInput>,
    /** The step's projection of the candidate document. */
    pub document: document::ScriptDocument,
    /** Origin storage candidates. */
    pub storage: v9::StorageState,
    /** Preferred languages for navigator.languages. */
    pub languages: Vec<String>,
    /** URL, viewport, clock, geometry and the facts the page sees. */
    pub environment: v9::PageEnvironment,
    /** Every page-owned lease this process may keep states for. */
    pub resident_contexts: Vec<String>,
    /** The resident state the step starts from; none for a first load. */
    pub base: Option<u64>,
    /** The committed state kept beside `base` for an abort. */
    pub keep: Option<u64>,
    /** What to run. */
    pub action: PageAction,
}

/** One typed page step on this engine thread, on V9's own stack, kept resident. */
pub(super) fn run_typed_page(call: TypedPage) -> EngineResult<v9::Execution> {
    admit_resident(
        &call.document.identity,
        Some(&call.resident_contexts),
        call.base,
        call.keep,
    )?;
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_eye::platform::NativeCanvas,
        languages: &call.languages,
        fragments: &document::DomFragments,
    };
    let input = v9::PageInput {
        sources: &call.sources.texts,
        source_keys: &call.sources.keys,
        document: call.document,
        storage: call.storage,
        host: &host,
        environment: &call.environment,
        modules: &call.modules,
    };
    let base = call.base;
    let state = || {
        base.map_or_else(
            || {
                Err(EngineError::invalid(
                    "This V9 page step needs committed state.",
                ))
            },
            resident_state,
        )
    };
    let mut execution = run_action(call.action, input, state, false)?;
    keep_resident(&mut execution, call.base, call.keep);
    Ok(execution)
}

/// Pages whose states this process keeps between steps. Each keeps only its committed state,
/// the last step's base and that step's result, so a 25 MB heap (YouTube) never crosses the
/// pipe and memory stays bounded; a page past the limit gets `state_missing` on its next step.
pub(super) const RESIDENT_PAGES: usize = super::residency::CONTEXTS;
/// The error a step gets when its base names a state this process no longer holds (it was
/// restarted, or dropped the page); the coordinator cannot resend it, so it reloads the page.
pub(super) const STATE_MISSING: &str = "state_missing";

/** One resident state. A step runs on its own copy, because a retry or an abort returns to the
original; the heap is persistent, so the copy shares every object the step does not write and
costs nothing to take. */
struct Resident {
    /** The state itself, never changed. */
    state: std::sync::Arc<v9::VmState>,
}

impl Resident {
    /** Keep `state`. */
    fn new(state: v9::VmState) -> Self {
        Self {
            state: std::sync::Arc::new(state),
        }
    }
    /** A copy to run a step on. */
    fn copy(&mut self) -> v9::VmState {
        v9::VmState::clone(&self.state)
    }
}

/** Disposal is synchronous so retired versions cannot accumulate in unbounded detached threads. */
fn discard(states: Vec<(u64, Resident)>) {
    drop(states);
}

/** Validate ownership completely before releasing old states or executing a new context. */
fn admit_resident(
    identity: &str,
    contexts: Option<&[String]>,
    base: Option<u64>,
    keep: Option<u64>,
) -> EngineResult<()> {
    let contexts =
        contexts.ok_or_else(|| EngineError::invalid("Missing resident context leases."))?;
    if contexts.len() > RESIDENT_PAGES {
        return Err(EngineError::limit(
            "resident script contexts",
            RESIDENT_PAGES,
        ));
    }
    let unique: std::collections::BTreeSet<_> = contexts.iter().collect();
    if unique.len() != contexts.len()
        || !contexts.iter().any(|held| held == identity)
        || contexts
            .iter()
            .any(|held| held.is_empty() || held.len() > crate::constants::ID_BYTES)
    {
        return Err(EngineError::invalid("Invalid resident context leases."));
    }
    PAGES.with_borrow_mut(|pages| {
        for token in [base, keep].into_iter().flatten() {
            let owner = pages
                .iter()
                .find(|page| page.states.iter().any(|(held, _)| *held == token));
            match owner {
                Some(owner) if owner.identity == identity => (),
                Some(_) => {
                    return Err(EngineError::invalid(
                        "Resident token belongs to another context.",
                    ));
                }
                None => {
                    return Err(EngineError::new(
                        STATE_MISSING,
                        "The V9 service no longer holds this page's script state.",
                        "Reload the page.",
                    ));
                }
            }
        }
        pages.retain(|page| contexts.contains(&page.identity));
        Ok(())
    })
}

/** One page's resident states, by token. */
struct ResidentPage {
    /** The document identity the states belong to (a token when the page has none yet). */
    identity: String,
    /** At most the committed state, the last base and its result. */
    states: Vec<(u64, Resident)>,
}

thread_local! {
    /** Most recently used page first. */
    static PAGES: std::cell::RefCell<std::collections::VecDeque<ResidentPage>> = Default::default();
    /** The next token; it starts at a random point, so a restarted process misses old tokens. */
    static NEXT_TOKEN: std::cell::Cell<u64> = std::cell::Cell::new(token_seed());
}

/** A random starting token. */
fn token_seed() -> u64 {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = [0_u8; 8];
    let _ = SystemRandom::new().fill(&mut bytes);
    u64::from_le_bytes(bytes)
}

/** A copy of the resident state `token` names, for a step to run on (a retry runs on the same
base again, so the base itself stays untouched). */
fn resident_state(token: u64) -> EngineResult<v9::VmState> {
    PAGES
        .with_borrow_mut(|pages| {
            pages
                .iter_mut()
                .flat_map(|page| &mut page.states)
                .find(|(held, _)| *held == token)
                .map(|(_, resident)| resident.copy())
        })
        .ok_or_else(|| {
            EngineError::new(
                STATE_MISSING,
                "The V9 service no longer holds this page's script state.",
                "Reload the page.",
            )
        })
}

/** Keep the step's full state here under a new token and reply with its summary. */
fn keep_resident(execution: &mut v9::Execution, base: Option<u64>, keep: Option<u64>) {
    let token = NEXT_TOKEN.get();
    NEXT_TOKEN.set(token.wrapping_add(1));
    let state = std::mem::take(&mut execution.state);
    execution.state = state.summary();
    execution.state_token = Some(token);
    let identity = match state.identity() {
        "" => token.to_string(),
        identity => identity.to_string(),
    };
    PAGES.with_borrow_mut(|pages| {
        let mut page = pages
            .iter()
            .position(|page| page.identity == identity)
            .and_then(|index| pages.remove(index))
            .unwrap_or(ResidentPage {
                identity,
                states: Vec::new(),
            });
        let (kept, dropped) = std::mem::take(&mut page.states)
            .into_iter()
            .partition(|(held, _)| Some(*held) == base || Some(*held) == keep);
        page.states = kept;
        discard(dropped);
        page.states.push((token, Resident::new(state)));
        pages.push_front(page);
        debug_assert!(pages.len() <= RESIDENT_PAGES);
    });
}

/// Distinct source lists the V9 process keeps: the current page's, its frames' and the page
/// just left, so memory stays bounded by four times SCRIPT_TOTAL_BYTES.
pub(super) const RESIDENT_SOURCES: usize = 4;
/// The error a step gets when its digest names sources this process no longer holds (it was
/// restarted, or evicted them); the coordinator then sends the sources themselves.
pub(super) const SOURCES_MISSING: &str = "sources_missing";

/** A page's scripts with their code-cache keys, hashed once when they arrive: V9 hashed every
source on every step, and YouTube's 10 MB of scripts took longer to hash than many steps ran. */
#[derive(Debug)]
pub(super) struct ResidentSources {
    pub texts: Vec<String>,
    pub keys: Vec<v9::SourceKey>,
}

impl ResidentSources {
    /** Sources with their keys. */
    pub(super) fn new(texts: Vec<String>) -> Self {
        let keys = texts.iter().map(|text| v9::source_key(text)).collect();
        Self { texts, keys }
    }
}

thread_local! {
    /** Most recently used first; content-addressed, so pages never see each other's state. */
    static RESIDENT: std::cell::RefCell<std::collections::VecDeque<(String, std::rc::Rc<ResidentSources>)>> =
        Default::default();
}

/**
 * The step's sources: the ones sent (checked against their digest, then kept), or the resident
 * ones the digest names. The digest is recomputed here, so a page cannot plant code under
 * another page's digest.
 */
fn resident_sources(
    sources: Vec<String>,
    digest: Option<String>,
) -> EngineResult<std::rc::Rc<ResidentSources>> {
    let Some(digest) = digest else {
        // Sent on every call, so hashed by V9 on every call: nothing to keep keys for.
        return Ok(std::rc::Rc::new(ResidentSources {
            texts: sources,
            keys: Vec::new(),
        }));
    };
    RESIDENT.with_borrow_mut(|resident| {
        let position = resident.iter().position(|(key, _)| *key == digest);
        if sources.is_empty() {
            let entry = position
                .and_then(|index| resident.remove(index))
                .ok_or_else(|| {
                    EngineError::new(
                        SOURCES_MISSING,
                        "The V9 service does not hold this page's scripts.",
                        "Send the page's sources with the step.",
                    )
                })?;
            let sources = std::rc::Rc::clone(&entry.1);
            resident.push_front(entry);
            return Ok(sources);
        }
        if sources_digest(&sources) != digest {
            return Err(EngineError::invalid(
                "The page's sources do not match their digest.",
            ));
        }
        if let Some(index) = position {
            resident.remove(index);
        }
        let sources = std::rc::Rc::new(ResidentSources::new(sources));
        resident.push_front((digest, std::rc::Rc::clone(&sources)));
        resident.truncate(RESIDENT_SOURCES);
        Ok(sources)
    })
}

thread_local! {
    /** Module inputs, most recently used first, by digest; at most RESIDENT_SOURCES. */
    static RESIDENT_MODULES: std::cell::RefCell<std::collections::VecDeque<(String, std::rc::Rc<v9::ModuleInput>)>> =
        Default::default();
}

/**
 * The step's module input: the one sent (checked against its digest, then kept), or the
 * resident one the digest names (`sources_missing` when this process no longer holds it).
 */
fn resident_modules(
    modules: v9::ModuleInput,
    digest: Option<String>,
) -> EngineResult<std::rc::Rc<v9::ModuleInput>> {
    let Some(digest) = digest else {
        return Ok(std::rc::Rc::new(modules));
    };
    RESIDENT_MODULES.with_borrow_mut(|resident| {
        let position = resident.iter().position(|(key, _)| *key == digest);
        if modules.entries.is_empty() && modules.sources.is_empty() {
            let entry = position
                .and_then(|index| resident.remove(index))
                .ok_or_else(|| {
                    EngineError::new(
                        SOURCES_MISSING,
                        "The V9 service does not hold this page's modules.",
                        "Send the page's modules with the step.",
                    )
                })?;
            let modules = std::rc::Rc::clone(&entry.1);
            resident.push_front(entry);
            return Ok(modules);
        }
        if modules_digest(&modules) != digest {
            return Err(EngineError::invalid(
                "The page's modules do not match their digest.",
            ));
        }
        if let Some(index) = position {
            resident.remove(index);
        }
        let modules = std::rc::Rc::new(modules);
        resident.push_front((digest, std::rc::Rc::clone(&modules)));
        resident.truncate(RESIDENT_SOURCES);
        Ok(modules)
    })
}

/// SHA-256 over a module input's encoding, as lowercase hex; the coordinator and this process
/// encode the same value the same way, so equal digests mean equal inputs.
pub(super) fn modules_digest(modules: &v9::ModuleInput) -> String {
    let encoded = serde_json::to_vec(modules).unwrap_or_default();
    ring::digest::digest(&ring::digest::SHA256, &encoded)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// SHA-256 over each source's length and bytes, in order, as lowercase hex.
pub(super) fn sources_digest(sources: &[String]) -> String {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    for source in sources {
        context.update(&(source.len() as u64).to_le_bytes());
        context.update(source.as_bytes());
    }
    context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/** Headless policy stays literal-loopback only. */
fn fetch(params: Value) -> EngineResult<Value> {
    let (source, charset) = network::fetch_decoded(&parameters::<Fetch>(params)?.url)?;
    Ok(json!({"source": source, "charset": charset}))
}
/** Desktop policy validates every exact destination inside this process. */
fn web_fetch(params: Value) -> EngineResult<Value> {
    let input: WebFetch = parameters(params)?;
    serialize(network::fetch_web(&input.url, &input.fixture_origins)?)
}
/** Empties Relay's HTTP cache. It is memory only: engines may write only the shared engine
scratch folder (packaging/sandbox/engine.sb), and a disk cache there could be planted by the
document or V9 engine, so `set_cache_directory` is deliberately not called. */
fn cache_clear(params: Value) -> EngineResult<Value> {
    let _: Empty = parameters(params)?;
    network::clear_cache();
    Ok(json!({"cleared":true}))
}
/** Drops this process's cached copies of URLs an unsafe request in another network process
changed (page fetches run on the loader's own processes, each with its own memory cache). */
fn cache_forget(params: Value) -> EngineResult<Value> {
    let input: Forget = parameters(params)?;
    if input.urls.len() > FORGET_URLS {
        return Err(EngineError::limit("forgotten URLs", FORGET_URLS));
    }
    network::forget(&input.urls);
    Ok(json!({"forgotten":true}))
}
/** URLs one `network.cache_forget` may name: an unsafe request changes one resource. */
const FORGET_URLS: usize = 16;
/** Fetches a bounded batch concurrently through Relay's ordinary prepare and transmit path. GETs
only (Relay refuses others per request); redirects come back unfollowed. */
fn fetch_batch(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, run_batch)
}
/** The batch over its decoded parameters, outcomes in request order. */
fn run_batch(input: FetchBatch) -> EngineResult<Vec<BatchOutcome>> {
    if input.requests.len() > super::resources::FETCH_BATCH {
        return Err(EngineError::limit(
            "batched requests",
            super::resources::FETCH_BATCH,
        ));
    }
    let outcomes = network::fetch_batch(input.requests, &input.fixture_origins, input.remaining_ms)
        .into_iter()
        .map(|result| match result {
            Ok(response) => BatchOutcome::Response(response),
            Err(error) => BatchOutcome::Error(error),
        })
        .collect::<Vec<_>>();
    Ok(outcomes)
}
/** Markup semantics are owned by the document service. */
fn parse(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Parse| {
        if input.xml {
            document::parse_xml(&input.source, input.charset.as_deref())
        } else {
            document::parse_with_charset(&input.source, input.charset.as_deref())
        }
    })
}
/** All proposed targets are checked before any mutation escapes. */
fn apply(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Apply| {
        let applied = document::apply_changes(&input.document, &input.changes)?;
        if input.script_changes.is_empty() {
            return Ok(applied);
        }
        document::apply_script_changes(&applied, &input.script_changes)
    })
}
/** Legacy abstract geometry remains available to headless clients without intermediate JSON trees. */
fn layout(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Snapshot| {
        document::validate_content(&input.document)?;
        Ok(document::layout(&input.document))
    })
}
/** Native point layout is computed once, independently of the window. */
fn render(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, run_render)
}
/** Render over decoded parameters. */
fn run_render(input: Render) -> EngineResult<document::DisplayList> {
    let context = input.context.unwrap_or_else(|| document::RenderContext {
        width: input
            .width
            .unwrap_or(crate::constants::DEFAULT_VIEWPORT_WIDTH),
        ..Default::default()
    });
    if input.width.is_some_and(|width| width != context.width) {
        return Err(EngineError::invalid("Conflicting native viewport widths."));
    }
    document::render_scrolled(&input.document, context, &input.scroll)
}
/** Type actions and human input apply literal values directly from decoded parameters. */
fn value(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: SetValue| {
        document::set_value(&input.document, input.local_id, &input.value)
    })
}
/** Legacy execution evaluates source directly without intermediate JSON allocation. */
fn execute(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Execute| v9::execute(&input.source))
}
/** Load-time scripts initialize VM state straight from typed frame parameters. */
fn load(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Load| {
        v9::load_scripts_with_storage(&input.sources, &input.inputs, input.storage)
    })
}
/** Event handlers execute under committed state straight from typed request frames. */
fn event(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: Event| {
        v9::event_scripts_with_storage(
            &input.sources,
            &input.handler,
            input.state,
            &input.inputs,
            input.storage,
        )
    })
}
/** Document scripts load into transactional VM state directly from typed frames. */
fn document_load(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: DocumentLoad| {
        v9::load_document_scripts_with(
            &input.sources,
            input.document,
            input.storage,
            &host(&input.languages),
        )
    })
}
/** Document click handlers execute under transactional state directly from typed frames. */
fn document_click(frame: &[u8], id: &str) -> EngineResult<Vec<u8>> {
    typed(frame, id, |input: DocumentClick| {
        v9::click_document_scripts_with(
            &input.sources,
            input.target,
            input.state,
            input.document,
            input.storage,
            &host(&input.languages),
        )
    })
}

/** Eye's pure fragment parser runs inside the V9 process; parsing grants no authority. */
fn host(languages: &[String]) -> v9::DocumentHost<'_> {
    v9::DocumentHost {
        imports: None,
        canvas: &mars_eye::platform::NativeCanvas,
        languages,
        fragments: &document::Fragments,
    }
}

/** Serialization failures are explicit rather than successful null values. */
fn serialize(value: impl serde::Serialize) -> EngineResult<Value> {
    serde_json::to_value(value).map_err(|_| EngineError::invalid("Cannot serialize engine result."))
}

#[cfg(test)]
mod tests {
    use super::{
        RESIDENT_PAGES, SOURCES_MISSING, STATE_MISSING, keep_resident, modules_digest,
        resident_modules, resident_sources, resident_state, sources_digest,
    };
    use crate::v9;
    use serde_json::json;

    /** A step's outcome whose state belongs to the page `identity`. */
    fn execution(identity: &str) -> v9::Execution {
        serde_json::from_value(json!({
            "changes": [],
            "storage": v9::StorageState::default(),
            "state": {"page": {"identity": identity}}
        }))
        .unwrap()
    }

    /** Keep a step's outcome resident and return its token. */
    fn kept(identity: &str, base: Option<u64>, keep: Option<u64>) -> u64 {
        let mut execution = execution(identity);
        keep_resident(&mut execution, base, keep);
        execution.state_token.unwrap()
    }

    /** A step's state stays in the process under its token; the reply carries the token. */
    #[test]
    fn step_state_stays_resident_under_its_token() {
        let token = kept("page", None, None);
        assert_eq!(resident_state(token).unwrap().identity(), "page");
        assert_eq!(resident_state(token + 1).unwrap_err().code, STATE_MISSING);
    }

    /** A page keeps only its committed state, the last step's base and that step's result, so
    an abort can return to the committed state and a retry can rerun from the base. */
    #[test]
    fn page_keeps_only_committed_base_and_result() {
        let committed = kept("page", None, None);
        let first = kept("page", Some(committed), Some(committed));
        let second = kept("page", Some(first), Some(committed));
        assert!(resident_state(first).is_ok());
        let third = kept("page", Some(second), Some(committed));
        assert_eq!(resident_state(first).unwrap_err().code, STATE_MISSING);
        for token in [committed, second, third] {
            assert!(resident_state(token).is_ok());
        }
    }

    /** Excess admission is refused before any live context or rollback token is removed. */
    #[test]
    fn resident_pages_are_bounded() {
        let contexts: Vec<_> = (0..RESIDENT_PAGES).map(|n| format!("page-{n}")).collect();
        let mut tokens = Vec::new();
        for context in &contexts {
            super::admit_resident(context, Some(&contexts), None, None).unwrap();
            tokens.push(kept(context, None, None));
        }
        let mut excess = contexts.clone();
        excess.push("extra".into());
        assert_eq!(
            super::admit_resident("extra", Some(&excess), None, None)
                .unwrap_err()
                .code,
            "limit_exceeded"
        );
        for token in tokens {
            assert!(resident_state(token).is_ok());
        }
    }

    /** Invalid snapshots and cross-context tokens cannot discard another context's state. */
    #[test]
    fn invalid_leases_leave_resident_states_untouched() {
        let token = kept("original", None, None);
        for snapshot in [
            None,
            Some(vec!["other".into()]),
            Some(vec!["original".into(), "original".into()]),
        ] {
            assert!(
                super::admit_resident("original", snapshot.as_deref(), Some(token), None).is_err()
            );
            assert!(resident_state(token).is_ok());
        }
        assert!(
            super::admit_resident("other", Some(&["other".into()]), Some(token), None).is_err()
        );
        assert!(resident_state(token).is_ok());
        super::admit_resident("other", Some(&["other".into()]), None, None).unwrap();
        assert_eq!(resident_state(token).unwrap_err().code, STATE_MISSING);
    }

    /** Modules cross once, are found by digest after, and cannot be planted under a wrong one. */
    #[test]
    fn resident_modules_are_content_addressed() {
        let modules: v9::ModuleInput = serde_json::from_value(json!({
            "sources": [{"url": "https://a.test/m.js", "source": "export const a = 1;"}]
        }))
        .unwrap();
        let digest = modules_digest(&modules);
        let missing = resident_modules(Default::default(), Some(digest.clone())).unwrap_err();
        assert_eq!(missing.code, SOURCES_MISSING);
        let forged: v9::ModuleInput = serde_json::from_value(json!({
            "sources": [{"url": "https://a.test/m.js", "source": "evil()"}]
        }))
        .unwrap();
        assert_ne!(
            resident_modules(forged, Some(digest.clone()))
                .unwrap_err()
                .code,
            SOURCES_MISSING
        );
        resident_modules(modules.clone(), Some(digest.clone())).unwrap();
        let held = resident_modules(Default::default(), Some(digest)).unwrap();
        assert_eq!(held.sources[0].source, modules.sources[0].source);
    }

    /** Sources cross once, are found by digest after, and cannot be planted under a wrong one. */
    #[test]
    fn resident_sources_are_content_addressed() {
        let sources = vec!["let a = 1;".to_owned(), "a++".to_owned()];
        let digest = sources_digest(&sources);
        let missing = resident_sources(Vec::new(), Some(digest.clone())).unwrap_err();
        assert_eq!(missing.code, SOURCES_MISSING);
        let forged = resident_sources(vec!["evil()".into()], Some(digest.clone())).unwrap_err();
        assert_ne!(forged.code, SOURCES_MISSING);
        assert_eq!(
            resident_sources(sources.clone(), Some(digest.clone()))
                .unwrap()
                .texts,
            sources
        );
        assert_eq!(
            resident_sources(Vec::new(), Some(digest)).unwrap().texts,
            sources
        );
        assert_ne!(
            sources_digest(&["ab".into(), "c".into()]),
            sources_digest(&["a".into(), "bc".into()])
        );
    }
}
