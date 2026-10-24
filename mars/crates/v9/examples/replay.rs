/**
 * Replays `v9.page` frames the app dumped (`MARS_PERF_DUMP=<folder>` on a desktop run) with
 * scaled heap bounds, printing each step's console, failures, unsupported APIs and the live
 * counts the page needs, so a real site's requirements are measured rather than guessed.
 * `--scale N` multiplies the heap bounds. Requires `measure`.
 */
use mars_v9::{
    DocumentHost, Execution, ModuleInput, PageEnvironment, PageEvent, PageInput,
    SimpleFragmentParser, StorageState, VmState, document::ScriptDocument,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/** The v9.page parameters, mirrored from the service schema. */
#[derive(Deserialize)]
struct PageCall {
    sources: Vec<String>,
    #[serde(default)]
    sources_digest: Option<String>,
    document: ScriptDocument,
    #[serde(default)]
    storage: StorageState,
    #[serde(default)]
    languages: Vec<String>,
    environment: PageEnvironment,
    #[serde(default)]
    modules: ModuleInput,
    #[serde(default)]
    state: Option<VmState>,
    action: Value,
}

/** The frame files of one folder, in request order. */
fn frames(folder: &str) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .expect("dump folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with("v9.page.json"))
        .collect();
    files.sort();
    files
}

/** One step's console, failures and effects. */
fn report(label: &str, execution: &Execution) {
    println!("-- {label}");
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
    for request in &execution.effects.requests {
        println!(
            "[request {}] {} {} {} mode {} credentials {} body {} bytes",
            request.id,
            request.kind,
            request.method,
            request.url,
            request.mode,
            request.credentials,
            request.body.as_ref().map_or(0, String::len)
        );
    }
    for request in &execution.script_requests {
        println!("[script request] {}", request.url);
    }
    println!(
        "[effects] {} requests, navigation {:?}, next timer {:?}, {} script changes",
        execution.effects.requests.len(),
        execution.effects.navigation.as_ref().map(|n| &n.url),
        execution.effects.next_timer_ms,
        execution.script_changes.len()
    );
    if let Some(counts) = mars_v9::measure::last_counts() {
        println!(
            "[counts] objects {} functions {} properties {} units {} environments {} bindings {} state bytes {}",
            counts.0, counts.1, counts.2, counts.3, counts.4, counts.5, counts.6
        );
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut scale = 1;
    let mut chain = false;
    loop {
        match args.first().map(String::as_str) {
            Some("--scale") => {
                scale = args[1].parse().unwrap_or(1);
                args.drain(0..2);
            }
            // Carry each step's committed state into the next instead of the dumped one, so a
            // probe added to the dumped sources is still there when a later step runs.
            Some("--chain") => {
                chain = true;
                args.remove(0);
            }
            _ => break,
        }
    }
    let folder = args.first().expect("a dump folder");
    let mut resident: HashMap<String, Vec<String>> = HashMap::new();
    let mut carried: Option<VmState> = None;
    let mut carried_document: Option<ScriptDocument> = None;
    let mut carried_text: BTreeMap<usize, String> = BTreeMap::new();
    for path in frames(folder) {
        let frame = std::fs::read(&path).expect("frame");
        let request: Value = serde_json::from_slice(&frame).expect("request");
        let mut call: PageCall =
            serde_json::from_value(request["params"].clone()).expect("v9.page params");
        if let Some(digest) = &call.sources_digest {
            if call.sources.is_empty() {
                call.sources = resident.get(digest).cloned().unwrap_or_default();
            } else {
                resident.insert(digest.clone(), call.sources.clone());
            }
        }
        let host = DocumentHost {
            languages: &call.languages,
            fragments: &SimpleFragmentParser,
        };
        // Chaining carries the document and node text too, replaying each step's changes the
        // way the app's document engine does, so the projection matches the carried state.
        if call.action["kind"].as_str() == Some("load") {
            carried_document = None;
            carried_text.clear();
            carried = None;
        }
        let document = match (&chain, carried_document.take()) {
            (true, Some(document)) => document,
            _ => call.document.clone(),
        };
        let mut environment = call.environment.clone();
        if chain && !carried_text.is_empty() {
            environment.node_text = carried_text.clone();
        }
        let input = PageInput {
            sources: &call.sources,
            document: document.clone(),
            storage: call.storage,
            host: &host,
            environment: &environment,
            modules: &call.modules,
        };
        let state = if chain {
            carried.take().or(call.state.clone()).unwrap_or_default()
        } else {
            call.state.clone().unwrap_or_default()
        };
        let kind = call.action["kind"].as_str().unwrap_or_default().to_string();
        let started = std::time::Instant::now();
        let result = match kind.as_str() {
            "load" => mars_v9::measure::load_page_scaled(input, scale),
            "event" => {
                let event: PageEvent =
                    serde_json::from_value(call.action["event"].clone()).expect("event");
                mars_v9::measure::page_event_scaled(input, state, event, scale)
            }
            "modules" => mars_v9::load_modules(input, state),
            "resume" => {
                let responses: Vec<mars_v9::HostResponse> =
                    serde_json::from_value(call.action["responses"].clone()).expect("responses");
                mars_v9::resume_requests(input, state, responses)
            }
            "input" => mars_v9::input_page(
                input,
                state,
                call.action["target"].as_u64().unwrap_or_default() as usize,
                call.action["value"].as_str().map(str::to_owned),
            ),
            "click" => mars_v9::measure::click_page_scaled(
                input,
                state,
                call.action["target"].as_u64().unwrap_or_default() as usize,
                scale,
            ),
            "timers" => mars_v9::measure::run_timers_scaled(
                input,
                state,
                call.action["now_ms"].as_f64().unwrap_or_default(),
                call.action["budget"].as_u64().unwrap_or_default() as usize,
                scale,
            ),
            _ => continue,
        };
        let label = format!(
            "{} {kind} ({:.0} ms)",
            path.display(),
            started.elapsed().as_secs_f64() * 1000.0
        );
        match result {
            Ok(execution) => {
                report(&label, &execution);
                if chain {
                    let mut next = document;
                    let mut text = environment.node_text.clone();
                    if let Err(error) =
                        mars_v9::replay(&mut next, &execution, &SimpleFragmentParser, &mut text)
                    {
                        println!("replay failed: {}", error.message);
                    }
                    carried_document = Some(next);
                    carried_text = text;
                }
                carried = Some(execution.state);
            }
            Err(error) => println!("-- {label}\nerror {}: {}", error.code, error.message),
        }
    }
}
