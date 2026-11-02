/**
 * Replays real engine requests in-process and splits each one's cost into wire decode, engine
 * work and reply encode, so a slow page can be attributed to JSON or to Eye or V9 themselves.
 * Capture a page with `MARS_PERF_DUMP=<folder>` set on a desktop run, then
 * `MARS_REPLAY=<folder> cargo test --release --test perf_replay -- --nocapture`. Without
 * MARS_REPLAY the test does nothing, so the ordinary suite stays hermetic. MARS_REPLAY_VERBOSE
 * also prints each V9 step's full console, failures and unsupported APIs; MARS_REPLAY_CHAIN
 * carries each step's state and document into the next, and MARS_REPLAY_PROBE appends a script
 * file to the page's first source so it can report from inside the page.
 */
use mars::document::{self, Document, RenderContext};
use mars::protocol::Request;
use mars::v9;
use serde::Deserialize;
use serde_json::Value;
use std::time::Instant;

/** The v9.page parameters, mirrored from the service schema. */
#[derive(Deserialize)]
struct PageCall {
    sources: Vec<String>,
    /** Later steps name the sources by digest instead of sending them (`sources_digest`). */
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
    state: Option<v9::VmState>,
    action: Value,
}

/** document.styles parameters. */
#[derive(Deserialize)]
struct Styles {
    document: Document,
    sheets: Vec<String>,
    #[serde(default)]
    context: Option<RenderContext>,
}

/** document.render parameters. */
#[derive(Deserialize)]
struct Render {
    document: Document,
    #[serde(default)]
    context: Option<RenderContext>,
}

/** Milliseconds since `start`. */
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/** What a chained replay carries from one V9 step to the next instead of the dumped values:
the committed state, the projection with every step's changes replayed, and the node text. With
MARS_REPLAY_CHAIN set the steps form one run again, so a probe script appended to the page's
first source (MARS_REPLAY_PROBE names its file) can print what the page holds at any later step. */
#[derive(Default)]
struct Chain {
    on: bool,
    probe: Option<String>,
    state: Option<v9::VmState>,
    document: Option<document::ScriptDocument>,
    node_text: std::collections::BTreeMap<usize, String>,
}

impl Chain {
    /** Read the chain settings from the environment. */
    fn from_env() -> Self {
        Self {
            on: std::env::var_os("MARS_REPLAY_CHAIN").is_some(),
            probe: std::env::var_os("MARS_REPLAY_PROBE")
                .and_then(|path| std::fs::read_to_string(path).ok()),
            ..Self::default()
        }
    }
}

/** One V9 step: typed decode, the engine call, and the reply's encode. Sources arrive once per
page and are named by digest after, as on the wire, so `resident` keeps them between steps. */
fn page(
    params: Value,
    resident: &mut std::collections::HashMap<String, Vec<String>>,
    chain: &mut Chain,
) -> (f64, f64, f64, usize) {
    let started = Instant::now();
    let mut call: PageCall = serde_json::from_value(params).expect("v9.page params");
    if let Some(digest) = &call.sources_digest {
        if call.sources.is_empty() {
            call.sources = resident.get(digest).cloned().unwrap_or_default();
        } else {
            let many = call.sources.len() > 1;
            if let (Some(probe), Some(first)) = (&chain.probe, call.sources.first_mut())
                && many
            {
                first.push_str(probe);
            }
            resident.insert(digest.clone(), call.sources.clone());
        }
    }
    let decode = ms(started);
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_eye::platform::NativeCanvas,
        languages: &call.languages,
        fragments: &document::DomFragments,
    };
    // A load starts a page over: it takes the dumped document, never the one carried from the
    // page before it.
    let loading = call.action["kind"].as_str() == Some("load");
    if loading {
        chain.document = None;
        chain.node_text.clear();
        chain.state = None;
    }
    let document = match (chain.on, chain.document.take()) {
        (true, Some(document)) => document,
        _ => call.document.clone(),
    };
    let mut environment = call.environment.clone();
    if chain.on && !chain.node_text.is_empty() {
        environment.node_text = chain.node_text.clone();
    }
    let input = v9::PageInput {
        sources: &call.sources,
        source_keys: &[],
        module_keys: &[],
        document: document.clone(),
        storage: call.storage,
        host: &host,
        environment: &environment,
        modules: &call.modules,
    };
    let started = Instant::now();
    let state = match (chain.on, chain.state.take()) {
        (true, Some(state)) => state,
        _ => call.state.clone().unwrap_or_default(),
    };
    let result = match call.action["kind"].as_str().unwrap_or_default() {
        "load" => v9::load_page(input),
        "event" => {
            let event = serde_json::from_value(call.action["event"].clone()).expect("event");
            v9::page_event(input, state, event)
        }
        "timers" => v9::run_timers(
            input,
            state,
            call.action["now_ms"].as_f64().unwrap_or_default(),
            call.action["budget"].as_u64().unwrap_or_default() as usize,
        ),
        "resume" => {
            let responses =
                serde_json::from_value(call.action["responses"].clone()).expect("responses");
            v9::resume_requests(input, state, responses)
        }
        "modules" => v9::load_modules(input, state),
        "click" => v9::click_page(
            input,
            state,
            call.action["target"].as_u64().unwrap_or_default() as usize,
        ),
        "input" => v9::input_page(
            input,
            state,
            call.action["target"].as_u64().unwrap_or_default() as usize,
            call.action["value"].as_str().map(str::to_owned),
        ),
        _ => return (decode, 0.0, 0.0, 0),
    };
    let work = ms(started);
    let started = Instant::now();
    let bytes = result
        .map(|execution| {
            report(&execution);
            if chain.on {
                let mut next = document;
                let mut text = environment.node_text.clone();
                if let Err(error) =
                    v9::replay(&mut next, &execution, &document::DomFragments, &mut text)
                {
                    println!("replay failed: {}", error.message);
                }
                chain.document = Some(next);
                chain.node_text = text;
                chain.state = Some(execution.state.clone());
            }
            serde_json::to_vec(&execution).unwrap_or_default().len()
        })
        .unwrap_or_else(|error| {
            println!("v9.page error: {} {}", error.code, error.message);
            0
        });
    (decode, work, ms(started), bytes)
}

/** With MARS_REPLAY_VERBOSE set, prints a step's console output, failed scripts and unsupported
APIs in full, which the app's bounded console view truncates. */
fn report(execution: &v9::Execution) {
    if std::env::var_os("MARS_REPLAY_VERBOSE").is_none() {
        return;
    }
    for entry in &execution.effects.console {
        println!("[console {}] {}", entry.level, entry.text);
    }
    for failure in &execution.failed {
        println!(
            "[failed {}] {}: {}",
            failure.index, failure.error.code, failure.error.message
        );
    }
    for name in &execution.effects.unsupported {
        println!("[unsupported] {name}");
    }
    println!(
        "[effects] {} requests, navigation {:?}, next timer {:?}",
        execution.effects.requests.len(),
        execution.effects.navigation.as_ref().map(|n| &n.url),
        execution.effects.next_timer_ms
    );
}

/** One Eye call: typed decode, the engine call, and the reply's encode. */
fn eye(op: &str, params: Value) -> (f64, f64, f64, usize) {
    let started = Instant::now();
    match op {
        "document.styles" => {
            let input: Styles = serde_json::from_value(params).expect("styles params");
            let decode = ms(started);
            // MARS_REPLAY_REPEAT runs the cascade this many extra times, long enough to sample.
            let repeat = std::env::var("MARS_REPLAY_REPEAT")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            for _ in 0..repeat {
                let _ = document::compute_styles_with_context(
                    &input.document,
                    &input.sheets,
                    input.context.unwrap_or_default(),
                );
            }
            let started = Instant::now();
            let styled = document::compute_styles_with_context(
                &input.document,
                &input.sheets,
                input.context.unwrap_or_default(),
            )
            .expect("styles");
            let work = ms(started);
            // The same sheets over an empty document: the parse share of the work.
            let started = Instant::now();
            let empty = Document {
                nodes: Vec::new(),
                ..input.document.clone()
            };
            let _ = document::compute_styles_with_context(
                &empty,
                &input.sheets,
                input.context.unwrap_or_default(),
            );
            println!("document.styles sheets only\t{:.1}", ms(started));
            let started = Instant::now();
            let bytes = serde_json::to_vec(&styled).unwrap_or_default().len();
            (decode, work, ms(started), bytes)
        }
        "document.render" => {
            let input: Render = serde_json::from_value(params).expect("render params");
            let decode = ms(started);
            let started = Instant::now();
            let display =
                document::render_with_context(&input.document, input.context.unwrap_or_default())
                    .expect("render");
            let work = ms(started);
            let started = Instant::now();
            let bytes = serde_json::to_vec(&display).unwrap_or_default().len();
            (decode, work, ms(started), bytes)
        }
        "document.parse" => {
            let source = params["source"].as_str().unwrap_or_default().to_owned();
            let charset = params["charset"].as_str().map(str::to_owned);
            let decode = ms(started);
            let started = Instant::now();
            let parsed = document::parse_with_charset(&source, charset.as_deref()).expect("parse");
            let work = ms(started);
            let started = Instant::now();
            let bytes = serde_json::to_vec(&parsed).unwrap_or_default().len();
            (decode, work, ms(started), bytes)
        }
        _ => (0.0, 0.0, 0.0, 0),
    }
}

/** Prints one line per replayed request: op, frame bytes, frame parse, typed decode, work, encode. */
#[test]
fn replay_dumped_engine_requests() {
    let Some(folder) = std::env::var_os("MARS_REPLAY") else {
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .expect("dump folder")
        .flatten()
        .map(|entry| entry.path())
        .collect();
    files.sort();
    println!("op\tframe_bytes\tframe_ms\tdecode_ms\twork_ms\tencode_ms\treply_bytes");
    let mut resident = std::collections::HashMap::new();
    let mut chain = Chain::from_env();
    for path in files {
        let frame = std::fs::read(&path).expect("frame");
        let started = Instant::now();
        let request: Request = serde_json::from_slice(&frame).expect("request");
        let frame_ms = ms(started);
        let (decode, work, encode, bytes) = match request.op.as_str() {
            "v9.page" => page(request.params, &mut resident, &mut chain),
            op if op.starts_with("document.") => eye(op, request.params),
            _ => continue,
        };
        println!(
            "{}\t{}\t{frame_ms:.1}\t{decode:.1}\t{work:.1}\t{encode:.1}\t{bytes}",
            request.op,
            frame.len()
        );
    }
}
