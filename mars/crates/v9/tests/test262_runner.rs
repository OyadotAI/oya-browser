/** Conformance scoreboard for V9: runs tc39's test262 and V8's own mjsunit through V9's public page
API (`load_page` to run, `check` for parse-phase negatives) and writes per-directory counts.
Ignored by default; run it with

    TEST262=built-ins/Array cargo test --release --offline -p mars-v9 --test test262_runner -- --ignored --nocapture test262
    MJSUNIT=es6 cargo test --release --offline -p mars-v9 --features conformance --test test262_runner -- --ignored --nocapture mjsunit

`TEST262` / `MJSUNIT` are comma-separated path prefixes under `test/` (test262) or the mjsunit
folder; unset means everything. `TEST262_ROOT` / `MJSUNIT_ROOT` override the checkouts,
`CONF_OUT` the results folder, `CONF_JOBS` the thread count, `CONF_TIMEOUT_MS` the per-run timeout.
Set `CONF_REQUIRE_PASS=1` to fail the command after saving results when any executed case fails.
The opt-in `conformance` feature supplies d8.file fixture operations; the browser build omits it. */
mod common;
#[path = "support/mjsunit.rs"]
mod mjsunit_host;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

/** Each case's result slot, filled by whichever worker ran it. */
type Outcomes = Arc<Mutex<Vec<Option<Result<(), String>>>>>;

/** Features V9 does not claim (its README: no modules, with, async generators, for await,
SharedArrayBuffer, Atomics; language support stops at ES2023; Intl and Temporal are claimed),
plus host hooks it lacks. */
const UNCLAIMED: &[&str] = &[
    "SharedArrayBuffer",
    "Atomics",
    "Atomics.waitAsync",
    "Atomics.pause",
    "cross-realm",
    "IsHTMLDDA",
    "host-gc-required",
    "tail-call-optimization",
    "import-text",
    "import-assertions",
    "import-defer",
    "json-modules",
    "source-phase-imports",
    "source-phase-imports-module-source",
    "ShadowRealm",
    "decorators",
    "explicit-resource-management",
    "iterator-helpers",
    "iterator-sequencing",
    "joint-iteration",
    "set-methods",
    "array-grouping",
    "promise-with-resolvers",
    "promise-try",
    "resizable-arraybuffer",
    "arraybuffer-transfer",
    "immutable-arraybuffer",
    "regexp-v-flag",
    "regexp-modifiers",
    "regexp-duplicate-named-groups",
    "Float16Array",
    "Math.sumPrecise",
    "Error.isError",
    "upsert",
    "uint8array-base64",
    "json-parse-with-source",
    "String.prototype.isWellFormed",
    "String.prototype.toWellFormed",
    "await-dictionary",
    "canonical-tz",
    "legacy-regexp",
    "nonextensible-applies-to-private",
];
/** Whole test262 folders V9 does not claim. */
const UNCLAIMED_DIRS: &[&str] = &["staging"];
/** mjsunit folders that test d8, wasm or the sandbox rather than the language. */
const MJSUNIT_SKIP_DIRS: &[&str] = &["wasm", "shared-memory", "sandbox", "d8", "tools"];
/** Host definitions test262 expects (INTERPRETING.md): print for async tests and a small $262. */
const PRELUDE: &str = "var print = function (m) { console.log(String(m)); };\n\
var $262 = { global: globalThis, evalScript: function (s) { return (0, eval)(s); }, agent: {} };\n";

/** One test: its file, the source modes to run and what it must do. */
struct Case {
    rel: String,
    group: String,
    /** Full sources per mode ("strict" / "sloppy"). */
    runs: Vec<(&'static str, String)>,
    negative: Option<(String, String)>,
    is_async: bool,
    /** The host loader for tests that load modules from sibling files on disk. */
    host: Option<Host>,
}

/** test262's module host: the test file is the base URL, and module requests read files beside it. */
#[derive(Clone)]
struct Host {
    file: PathBuf,
    /** For `flags: [module]`: the harness, run as a classic script before the test module. */
    prelude: Option<String>,
    /** V8's separately parsed harness, declared prerequisites and actual test. */
    mjsunit: Option<mjsunit_host::Scripts>,
}

/** Frontmatter fields the runner reads. */
#[derive(Default)]
struct Meta {
    includes: Vec<String>,
    flags: Vec<String>,
    features: Vec<String>,
    negative: Option<(String, String)>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap())
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).unwrap_or(default)
}

fn out_dir() -> PathBuf {
    env_path(
        "CONF_OUT",
        PathBuf::from(
            "/private/tmp/claude-501/-Users-mk-Dev-oya-agentchrome/0608a5c7-c382-49f9-b32d-bb2fdc1f40c0/scratchpad/conf/test262",
        ),
    )
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, files);
        } else if path.extension().is_some_and(|e| e == "js") {
            files.push(path);
        }
    }
}

/** A YAML flow list `[a, b]` or the `- item` lines that follow the key. */
fn list(lines: &[&str], at: usize, rest: &str) -> Vec<String> {
    let rest = rest.trim();
    if let Some(inner) = rest.strip_prefix('[') {
        return inner
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    lines[at + 1..]
        .iter()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("- "))
        .map(|l| l[2..].trim().to_string())
        .collect()
}

fn meta(source: &str) -> Meta {
    let mut meta = Meta::default();
    let (Some(start), Some(end)) = (source.find("/*---"), source.find("---*/")) else {
        return meta;
    };
    let lines: Vec<&str> = source[start + 5..end].lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        match key {
            "includes" => meta.includes = list(&lines, i, rest),
            "flags" => meta.flags = list(&lines, i, rest),
            "features" => meta.features = list(&lines, i, rest),
            "negative" => {
                let field = |name: &str| {
                    lines[i + 1..]
                        .iter()
                        .take_while(|l| l.starts_with(' '))
                        .find_map(|l| l.trim().strip_prefix(name).map(|v| v.trim().to_string()))
                        .unwrap_or_default()
                };
                meta.negative = Some((field("phase:"), field("type:")));
            }
            _ => {}
        }
    }
    meta
}

/** Why a test262 file is skipped, or None to run it. */
fn skip262(rel: &str, meta: &Meta) -> Option<String> {
    let top = rel.split('/').next().unwrap_or("");
    if UNCLAIMED_DIRS.contains(&top) {
        return Some(format!("dir:{top}"));
    }
    if rel.contains("_FIXTURE") {
        return Some("fixture".into());
    }
    for flag in ["CanBlockIsTrue"] {
        if meta.flags.iter().any(|f| f == flag) {
            return Some(format!("flag:{flag}"));
        }
    }
    meta.features
        .iter()
        .find(|f| UNCLAIMED.contains(&f.as_str()))
        .map(|f| format!("feature:{f}"))
}

/** Directory bucket: the first two path parts (e.g. `built-ins/Array`). */
fn group(rel: &str, depth: usize) -> String {
    let parts: Vec<&str> = rel.split('/').collect();
    let n = depth.min(parts.len().saturating_sub(1)).max(1);
    if parts.len() == 1 {
        return "(root)".into();
    }
    parts[..n].join("/")
}

fn prefixes(key: &str) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn wanted(rel: &str, prefixes: &[String]) -> bool {
    prefixes.is_empty() || prefixes.iter().any(|p| rel.starts_with(p.as_str()))
}

/** Build the test262 cases plus the skip tally. */
fn cases262(root: &Path, skips: &mut BTreeMap<String, usize>) -> Vec<Case> {
    let harness = root.join("harness");
    let read = |name: &str| std::fs::read_to_string(harness.join(name)).unwrap_or_default();
    let base = format!("{PRELUDE}{}\n{}\n", read("assert.js"), read("sta.js"));
    let done = read("doneprintHandle.js");
    let filter = prefixes("TEST262");
    let mut files = Vec::new();
    walk(&root.join("test"), &mut files);
    files.sort();
    let mut cases = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(root.join("test"))
            .unwrap()
            .to_string_lossy()
            .to_string();
        if !wanted(&rel, &filter) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        let meta = meta(&source);
        if let Some(reason) = skip262(&rel, &meta) {
            *skips.entry(reason).or_default() += 1;
            continue;
        }
        let has = |f: &str| meta.flags.iter().any(|x| x == f);
        let is_async = has("async");
        let module = has("module");
        let body = if has("raw") || module {
            source.clone()
        } else {
            let mut prelude = base.clone();
            if is_async {
                prelude.push_str(&done);
                prelude.push('\n');
            }
            for include in &meta.includes {
                prelude.push_str(&read(include));
                prelude.push('\n');
            }
            format!("{prelude}{source}")
        };
        let mut runs = Vec::new();
        let mut prelude = None;
        if module {
            let mut harness = base.clone();
            if is_async {
                harness.push_str(&done);
                harness.push('\n');
            }
            for include in &meta.includes {
                harness.push_str(&read(include));
                harness.push('\n');
            }
            prelude = Some(harness);
            runs.push(("module", body));
        } else if !has("onlyStrict") && !has("noStrict") && !has("raw") {
            runs.push(("sloppy", body.clone()));
            runs.push(("strict", format!("\"use strict\";\n{body}")));
        } else if has("onlyStrict") {
            runs.push(("strict", format!("\"use strict\";\n{body}")));
        } else {
            runs.push(("sloppy", body));
        }
        cases.push(Case {
            group: group(&rel, 2),
            rel,
            runs,
            negative: meta.negative,
            is_async,
            host: (module || source.contains("import")).then(|| Host {
                file: file.clone(),
                prelude,
                mjsunit: None,
            }),
        });
    }
    cases
}

/** Build the mjsunit cases: mjsunit.js as the harness, sloppy mode, pass means no uncaught error. */
fn cases_mjsunit(root: &Path, skips: &mut BTreeMap<String, usize>) -> Vec<Case> {
    let harness = std::fs::read_to_string(root.join("mjsunit.js")).expect("mjsunit.js");
    let filter = prefixes("MJSUNIT");
    let mut files = Vec::new();
    walk(root, &mut files);
    files.sort();
    let mut cases = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        if rel == "mjsunit.js" || !wanted(&rel, &filter) {
            continue;
        }
        let top = rel.split('/').next().unwrap_or("");
        if rel.contains('/') && MJSUNIT_SKIP_DIRS.contains(&top) {
            *skips.entry(format!("dir:{top}")).or_default() += 1;
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        let flags: String = source
            .lines()
            .filter(|l| l.starts_with("// Flags:"))
            .collect();
        if flags.contains("--allow-natives-syntax") {
            *skips
                .entry("flag:--allow-natives-syntax".into())
                .or_default() += 1;
            continue;
        }
        if source.lines().any(|l| l.starts_with("// MODULE")) || rel.ends_with(".mjs") {
            *skips.entry("module".into()).or_default() += 1;
            continue;
        }
        cases.push(Case {
            group: group(&rel, 1),
            rel,
            runs: vec![("sloppy", source.clone())],
            negative: None,
            is_async: false,
            host: Some(Host {
                file: file.clone(),
                prelude: None,
                mjsunit: Some(mjsunit_host::Scripts::for_test(
                    root, &harness, &source, &file,
                )),
            }),
        });
    }
    cases
}

/** Run one source; Ok(()) on pass, Err(reason) on fail. */
fn run_one(
    source: &str,
    negative: &Option<(String, String)>,
    is_async: bool,
    host: Option<&Host>,
) -> Result<(), String> {
    if let Some(host) = host {
        return run_hosted(source, negative, is_async, host);
    }
    if let Some((phase, kind)) = negative
        && (phase == "parse" || phase == "early")
    {
        return match mars_v9::check(source) {
            Err(e)
                if kind != "SyntaxError"
                    || e.code == "script_syntax_error"
                    || e.message.contains("SyntaxError") =>
            {
                Ok(())
            }
            Err(e) => Err(format!("wrong parse error {kind}: {}", e.message)),
            Ok(()) => Err(format!("expected {phase} {kind}, parsed fine")),
        };
    }
    let host = common::host();
    let environment = common::environment();
    let sources = [source.to_string()];
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).map_err(|e| e.message)?;
    let failure = execution
        .failed
        .first()
        .map(|f| (f.error.code.clone(), f.error.message.clone()));
    if let Some((_, kind)) = negative {
        return match failure {
            Some((_, m)) if m.contains(&format!("Uncaught {kind}")) => Ok(()),
            Some((_, m)) => Err(format!("expected runtime {kind}: {m}")),
            None => Err(format!("expected runtime {kind}, ran fine")),
        };
    }
    if let Some((code, message)) = failure {
        return Err(format!("{code}: {message}"));
    }
    if is_async {
        let console = common::console(&execution);
        if !console.iter().any(|l| l == "Test262:AsyncTestComplete") {
            let detail = console.iter().find(|l| l.starts_with("Test262:")).cloned();
            return Err(detail.unwrap_or_else(|| "async test never called $DONE".into()));
        }
    }
    Ok(())
}

/** Run a test through the module host: classic or module, serving each round of module
requests from the files beside the test, as the app serves fetches. */
fn run_hosted(
    source: &str,
    negative: &Option<(String, String)>,
    is_async: bool,
    host: &Host,
) -> Result<(), String> {
    if let Some(scripts) = &host.mjsunit {
        return scripts.execute();
    }
    use mars_v9::{EntryKind, ModuleEntry, ModuleInput, ModuleSource};
    let url = url::Url::from_file_path(&host.file).map_err(|_| "bad test path".to_string())?;
    let mut environment = common::environment();
    environment.url = url.to_string();
    let mut input = ModuleInput::default();
    let sources: Vec<String> = match &host.prelude {
        Some(prelude) => {
            input.entries.push(ModuleEntry {
                index: 1,
                kind: EntryKind::Module,
                url: Some(url.to_string()),
            });
            vec![prelude.clone(), String::new()]
        }
        None => vec![source.to_string()],
    };
    let page = common::host();
    let mut execution = mars_v9::load_page(mars_v9::PageInput {
        modules: &input,
        ..common::input(&sources, &page, &environment)
    })
    .map_err(|e| e.message)?;
    let mut failures = Vec::new();
    let mut console = Vec::new();
    for _ in 0..64 {
        failures.extend(
            execution
                .failed
                .iter()
                .map(|f| (f.error.code.clone(), f.error.message.clone())),
        );
        console.extend(common::console(&execution));
        if execution.module_requests.is_empty() {
            break;
        }
        for request in &execution.module_requests {
            let text = if request.url == url.as_str() && host.prelude.is_some() {
                Some(source.to_string())
            } else {
                url::Url::parse(&request.url)
                    .ok()
                    .and_then(|u| u.to_file_path().ok())
                    .and_then(|path| std::fs::read_to_string(path).ok())
            };
            input.sources.push(ModuleSource {
                url: request.url.clone(),
                source: text,
                classic: false,
            });
        }
        execution = mars_v9::load_modules(
            mars_v9::PageInput {
                modules: &input,
                ..common::input(&sources, &page, &environment)
            },
            execution.state.clone(),
        )
        .map_err(|e| e.message)?;
    }
    // A module that rejects after a fetch round (top-level await) reports through the console.
    if host.prelude.is_some() {
        failures.extend(
            console
                .iter()
                .filter(|l| l.starts_with("Uncaught ") && !l.starts_with("Uncaught (in promise)"))
                .map(|l| ("module_rejected".to_string(), l.clone())),
        );
    }
    let failure = failures.into_iter().next();
    if let Some((_, kind)) = negative {
        return match failure {
            Some((code, m))
                if m.contains(kind.as_str())
                    || (kind == "SyntaxError" && code.contains("syntax")) =>
            {
                Ok(())
            }
            Some((_, m)) => Err(format!("expected {kind}: {m}")),
            None => Err(format!("expected {kind}, ran fine")),
        };
    }
    if let Some((code, message)) = failure {
        return Err(format!("{code}: {message}"));
    }
    if is_async && !console.iter().any(|l| l == "Test262:AsyncTestComplete") {
        let detail = console.iter().find(|l| l.starts_with("Test262:")).cloned();
        return Err(detail.unwrap_or_else(|| "async test never called $DONE".into()));
    }
    Ok(())
}

/** Run with a hard timeout; a hung or panicking run counts as a failure. */
fn run_guarded(
    source: String,
    negative: Option<(String, String)>,
    is_async: bool,
    host: Option<Host>,
    timeout: Duration,
) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    // ponytail: a timed-out worker thread is abandoned, not killed; V9's own script deadline ends it.
    thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let _ = tx.send(run_one(&source, &negative, is_async, host.as_ref()));
        })
        .map_err(|e| e.to_string())?;
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err("timeout".into()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("panic".into()),
    }
}

/** A failure message with locations and numbers stripped, to bucket similar failures. */
fn bucket(message: &str) -> String {
    let message = match message.find("Script execution failed at line") {
        Some(at) => format!(
            "{}{}",
            &message[..at],
            message[at..].split_once(": ").map_or("", |p| p.1)
        ),
        None => message.to_string(),
    };
    let mut out = String::new();
    let mut digits = false;
    for c in message.chars().take(110) {
        if c.is_ascii_digit() {
            if !digits {
                out.push('N');
            }
            digits = true;
        } else {
            digits = false;
            out.push(c);
        }
    }
    out
}

/** Run every mode of one case; the first failing mode is the reason. */
fn run_case(case: &Case, timeout: Duration) -> Result<(), String> {
    for (mode, source) in &case.runs {
        run_guarded(
            source.clone(),
            case.negative.clone(),
            case.is_async,
            case.host.clone(),
            timeout,
        )
        .map_err(|e| format!("[{mode}] {e}"))?;
    }
    Ok(())
}

/** Marks the worker's reply lines among libtest's own output. */
const REPLY: &str = "@@conf ";

/** A worker process: this same test binary running `conf_worker`, so an engine abort (V9 can
die on a huge allocation) loses one test, not the run. */
fn spawn_worker(suite: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "conf_worker",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CONF_SUITE", suite)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("worker spawns")
}

/** Send case `i` to the worker (spawning one if needed) and read its verdict. */
fn ask(child: &mut Option<Child>, suite: &str, i: usize) -> Result<(), String> {
    let worker = child.get_or_insert_with(|| spawn_worker(suite));
    let reply = writeln!(worker.stdin.as_mut().unwrap(), "{i}")
        .ok()
        .and_then(|_| read_reply(worker.stdout.as_mut().unwrap()));
    match reply {
        Some(Value::String(e)) => Err(e),
        Some(_) => Ok(()),
        None => {
            let _ = worker.kill();
            let status = worker.wait().ok();
            *child = None;
            Err(format!("crash: worker died ({status:?})"))
        }
    }
}

/** Read lines until a reply: null for a pass, a string for a failure; None when the worker died. */
fn read_reply(stdout: &mut ChildStdout) -> Option<Value> {
    // ponytail: one byte-at-a-time reader so no buffered bytes are lost between calls.
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        line.clear();
        loop {
            match std::io::Read::read(stdout, &mut byte) {
                Ok(1) if byte[0] == b'\n' => break,
                Ok(1) => line.push(byte[0]),
                _ => return None,
            }
        }
        let text = String::from_utf8_lossy(&line);
        // libtest's "test conf_worker ... " shares the first reply's line.
        if let Some(at) = text.find(REPLY) {
            return serde_json::from_str(&text[at + REPLY.len()..]).ok();
        }
    }
}

/** Worker mode: rebuild the suite's cases, then answer case indices from stdin. */
#[test]
#[ignore]
fn conf_worker() {
    let Ok(suite) = std::env::var("CONF_SUITE") else {
        return;
    };
    std::panic::set_hook(Box::new(|_| {}));
    let mut skips = BTreeMap::new();
    let cases = if suite == "mjsunit" {
        cases_mjsunit(&mjsunit_root(), &mut skips)
    } else {
        cases262(&test262_root(), &mut skips)
    };
    let timeout = timeout();
    let mut out = std::io::stdout();
    for line in BufReader::new(std::io::stdin())
        .lines()
        .map_while(Result::ok)
    {
        let Ok(i) = line.trim().parse::<usize>() else {
            continue;
        };
        let verdict = match cases.get(i).map(|c| run_case(c, timeout)) {
            Some(Ok(())) => Value::Null,
            Some(Err(e)) => Value::String(e),
            None => Value::String("no such case".into()),
        };
        let _ = writeln!(out, "{REPLY}{verdict}");
        let _ = out.flush();
    }
}

fn timeout() -> Duration {
    Duration::from_millis(
        std::env::var("CONF_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15_000),
    )
}

fn test262_root() -> PathBuf {
    env_path("TEST262_ROOT", home().join("Dev/upstream/test262"))
}

fn mjsunit_root() -> PathBuf {
    env_path("MJSUNIT_ROOT", home().join("Dev/upstream/v8/test/mjsunit"))
}

/** Run every case in parallel worker processes, print and write the scoreboard. */
fn score(suite: &'static str, cases: Vec<Case>, skips: BTreeMap<String, usize>) {
    std::panic::set_hook(Box::new(|_| {}));
    let started = Instant::now();
    let jobs = std::env::var("CONF_JOBS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });
    let cases = Arc::new(cases);
    let next = Arc::new(AtomicUsize::new(0));
    let outcomes: Outcomes = Arc::new(Mutex::new((0..cases.len()).map(|_| None).collect()));
    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let (cases, next, outcomes) = (cases.clone(), next.clone(), outcomes.clone());
            thread::spawn(move || {
                let mut child: Option<Child> = None;
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cases.len() {
                        break;
                    }
                    let result = ask(&mut child, suite, i);
                    outcomes.lock().unwrap()[i] = Some(result);
                    if i % 2000 == 0 {
                        eprintln!("{suite}: {i}/{}", cases.len());
                    }
                }
                if let Some(mut c) = child {
                    let _ = c.kill();
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let outcomes = outcomes.lock().unwrap();
    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut buckets: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for (case, outcome) in cases.iter().zip(outcomes.iter()) {
        let entry = groups.entry(case.group.clone()).or_default();
        match outcome.as_ref().unwrap() {
            Ok(()) => entry.0 += 1,
            Err(e) => {
                entry.1 += 1;
                *buckets.entry(bucket(e)).or_default() += 1;
                failures.push(json!({"test": case.rel, "error": e}));
            }
        }
    }
    let (pass, fail) = groups.values().fold((0, 0), |a, g| (a.0 + g.0, a.1 + g.1));
    println!(
        "\n{suite}: {pass} pass, {fail} fail, {} skipped ({:.1}s)",
        skips.values().sum::<usize>(),
        started.elapsed().as_secs_f64()
    );
    println!("{:<44} {:>6} {:>6} {:>7}", "group", "pass", "fail", "rate");
    for (name, (p, f)) in &groups {
        println!(
            "{name:<44} {p:>6} {f:>6} {:>6.1}%",
            100.0 * *p as f64 / (*p + *f).max(1) as f64
        );
    }
    let mut top: Vec<_> = buckets.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    println!("\ntop failure reasons:");
    for (reason, n) in top.iter().take(25) {
        println!("{n:>6}  {reason}");
    }
    println!("\nskipped:");
    for (reason, n) in &skips {
        println!("{n:>6}  {reason}");
    }
    let out = out_dir();
    std::fs::create_dir_all(&out).unwrap();
    let groups: BTreeMap<_, Value> = groups
        .iter()
        .map(|(k, (p, f))| (k.clone(), json!({"pass": p, "fail": f})))
        .collect();
    let top: Vec<Value> = top
        .iter()
        .take(100)
        .map(|(r, n)| json!({"reason": r, "count": n}))
        .collect();
    let file = out.join(format!("results-{suite}.json"));
    let body = json!({"suite": suite, "pass": pass, "fail": fail, "skipped": skips, "groups": groups, "top_failures": top, "failures": failures});
    std::fs::write(&file, serde_json::to_string_pretty(&body).unwrap()).unwrap();
    println!("results in {}", file.display());
    require_pass(
        fail,
        std::env::var("CONF_REQUIRE_PASS").as_deref() == Ok("1"),
    );
}

/** A strict conformance gate fails after writing the complete diagnostic scoreboard. */
fn require_pass(fail: usize, required: bool) {
    if required {
        assert_eq!(
            fail, 0,
            "Conformance gate requires every executed case to pass."
        );
    }
}

#[test]
/** Research scoring stays available, but the strict gate never treats case failures as success. */
fn strict_gate_reports_case_failures() {
    require_pass(0, true);
    require_pass(1, false);
    assert!(std::panic::catch_unwind(|| require_pass(1, true)).is_err());
}

/** tc39 test262 against V9. */
#[test]
#[ignore]
fn test262() {
    let root = test262_root();
    let mut skips = BTreeMap::new();
    let cases = cases262(&root, &mut skips);
    score("test262", cases, skips);
}

/** V8's mjsunit against V9. */
#[test]
#[ignore]
fn mjsunit() {
    let root = mjsunit_root();
    let mut skips = BTreeMap::new();
    let cases = cases_mjsunit(&root, &mut skips);
    score("mjsunit", cases, skips);
}

/** The frontmatter reader handles flow and block lists and the negative block, an unclaimed
feature skips a test, and Temporal (claimed since round 7) runs. */
#[test]
fn frontmatter_parses() {
    let m = meta(
        "/*---\nincludes: [a.js, b.js]\nflags:\n  - onlyStrict\nnegative:\n  phase: parse\n  type: SyntaxError\nfeatures: [ShadowRealm]\n---*/",
    );
    assert_eq!(m.includes, ["a.js", "b.js"]);
    assert_eq!(m.flags, ["onlyStrict"]);
    assert_eq!(m.negative, Some(("parse".into(), "SyntaxError".into())));
    assert_eq!(
        skip262("built-ins/x.js", &m).as_deref(),
        Some("feature:ShadowRealm")
    );
    let temporal = meta("/*---\nfeatures: [Temporal, Intl.NumberFormat-v3]\n---*/");
    assert_eq!(skip262("built-ins/Temporal/x.js", &temporal), None);
}
