#[path = "wpt_harness/driver.rs"]
mod driver;
/** WPT testharness.js scoreboard: serves the upstream WPT checkout over loopback, navigates the
full Mars runtime (V9, Eye and the runtime) to each testharness test, and counts the subtests.
`/resources/testharnessreport.js` is replaced by a reporter that writes every result, percent
encoded JSON, into a `data-r` attribute on an element the page appends; the runner reads it back
through page.nodes. `.any.js` and `.window.js` tests get the wrapper wptserve would generate.
Ignored by default; run it with

    WPT_HARNESS=dom,url cargo test --offline --test wpt_harness -- --ignored --nocapture

`WPT_HARNESS` is a comma-separated list of path prefixes under the WPT root (`dom`, or narrower,
like `dom/nodes`); unset means every suite in `SUITES`. `WPT_ROOT` overrides the checkout,
`WPT_OUT` the results folder, `WPT_JOBS` the thread count. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use support::{agent_call, desktop};

/** The suites the scoreboard covers when `WPT_HARNESS` names none. */
const SUITES: &[&str] = &[
    "dom",
    "html/dom",
    "html/syntax",
    "url",
    "encoding",
    "domparsing",
];
/** A loopback client that stalls never pins a server thread. */
const SOCKET_TIMEOUT: Duration = Duration::from_secs(5);
/** Extra awaits of the reporter's completion promise (each up to the runtime's await wall)
a page gets to finish after navigation settles. */
const SETTLE_POLLS: usize = 3;
/** A page slower than this is named on stderr as it finishes, so a long run shows progress. */
const SLOW_MS: u128 = 20_000;
/** The reporter that stands in for testharnessreport.js. */
const REPORTER: &str = r#"(function () {
  var finish;
  self.__wpt_done = new Promise(function (resolve) { finish = resolve; });
  /* A lone surrogate in a test name or message makes encodeURIComponent throw, and the throw
     inside the result callback dropped every later record of that test (encodeInto's names carry
     them). Lone surrogates become U+FFFD here; valid pairs pass through. */
  function safeText(text) {
    var out = '';
    for (var i = 0; i < text.length; i++) {
      var c = text.charCodeAt(i);
      var next = i + 1 < text.length ? text.charCodeAt(i + 1) : 0;
      if (c >= 0xd800 && c <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
        out += text.charAt(i) + text.charAt(i + 1);
        i++;
      } else {
        out += c >= 0xd800 && c <= 0xdfff ? '\ufffd' : text.charAt(i);
      }
    }
    return out;
  }
  function message(m) { return m == null ? null : safeText(String(m)).slice(0, 300); }
  /* One element per record: a single attribute holding every result so far grows with each
     result and outgrows the engine's text limit, which stopped the page's scripts. */
  function record(tag, payload) {
    var el = document.createElement(tag);
    el.setAttribute('data-r', encodeURIComponent(JSON.stringify(payload)));
    (document.body || document.documentElement).appendChild(el);
  }
  function report(test) {
    if (test.reported) return;
    test.reported = true;
    record('wpt-result', [safeText(String(test.name)), test.status, message(test.message)]);
  }
  setup({output: false});
  add_result_callback(report);
  add_completion_callback(function (tests, status) {
    tests.forEach(report);
    record('wpt-status', {harness: status.status, message: message(status.message)});
    finish(true);
  });
})();
"#;
/** testharness.js subtest statuses by number. */
const SUBTEST: &[&str] = &["pass", "fail", "timeout", "notrun", "precondition_failed"];
/** testharness.js harness statuses by number. */
const HARNESS: &[&str] = &["ok", "error", "timeout", "precondition_failed"];

/** The upstream WPT checkout, the document root the server serves. */
fn wpt_root() -> PathBuf {
    std::env::var_os("WPT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap()).join("Dev/upstream/wpt"))
}

/** Where results-<suite>.json lands. */
fn out_dir() -> PathBuf {
    std::env::var_os("WPT_OUT").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("/private/tmp/claude-501/-Users-mk-Dev-oya-agentchrome/0608a5c7-c382-49f9-b32d-bb2fdc1f40c0/scratchpad/conf/harness")
    })
}

// ---- static file server -------------------------------------------------------------------

/** Serve `root` on a loopback port, one thread per connection, until the process exits. */
fn serve(root: PathBuf) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let root = root.canonicalize().unwrap();
    thread::spawn(move || {
        for socket in listener.incoming().flatten() {
            let root = root.clone();
            thread::spawn(move || answer(socket, &root));
        }
    });
    url
}

/** The body for `path`: the reporter, a generated wrapper, or the file under `root`. */
fn body(root: &Path, path: &str) -> Option<(Vec<u8>, &'static str)> {
    if path == "/resources/testdriver-vendor.js" {
        return Some((driver::VENDOR.as_bytes().to_vec(), "text/javascript"));
    }
    if path == "/resources/testharnessreport.js" {
        return Some((REPORTER.as_bytes().to_vec(), "text/javascript"));
    }
    for (html, js) in [(".any.html", ".any.js"), (".window.html", ".window.js")] {
        if let Some(stem) = path.strip_suffix(html) {
            let source = file(root, &format!("{stem}{js}"))?;
            let name = format!("{}{js}", stem.rsplit('/').next().unwrap_or(stem));
            return Some((
                wrapper(&std::fs::read_to_string(source).ok()?, &name),
                "text/html; charset=utf-8",
            ));
        }
    }
    let file = file(root, path)?;
    Some((std::fs::read(&file).ok()?, mime(&file)))
}

/** The file under `root` that `path` names, never anything outside it. */
fn file(root: &Path, path: &str) -> Option<PathBuf> {
    root.join(percent_decode(path).trim_start_matches('/'))
        .canonicalize()
        .ok()
        .filter(|file| file.starts_with(root) && file.is_file())
}

/** The page wptserve generates for a `.any.js` or `.window.js` test: its META scripts, the
harness, then the test. */
fn wrapper(source: &str, name: &str) -> Vec<u8> {
    let mut page = String::from(
        "<!doctype html><meta charset=utf-8>\n<script>self.GLOBAL = {isWindow: () => true, isWorker: () => false, isShadowRealm: () => false};</script>\n",
    );
    for (key, value) in meta(source) {
        match key.as_str() {
            "title" => page += &format!("<title>{value}</title>\n"),
            "script" => page += &format!("<script src=\"{value}\"></script>\n"),
            _ => {}
        }
    }
    page = page.replacen(
        "\n",
        "\n<script src=/resources/testharness.js></script>\n<script src=/resources/testharnessreport.js></script>\n",
        1,
    );
    page += &format!("<div id=log></div>\n<script src=\"{name}\"></script>\n");
    page.into_bytes()
}

/** The `// META: key=value` lines at the top of a script test. */
fn meta(source: &str) -> Vec<(String, String)> {
    source
        .lines()
        .map_while(|line| line.trim().strip_prefix("// META:"))
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/** Answer one request, or 404. */
fn answer(mut socket: TcpStream, root: &Path) {
    let _ = socket.set_read_timeout(Some(SOCKET_TIMEOUT));
    let mut request = Vec::new();
    let mut chunk = [0; 4096];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        match socket.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => request.extend_from_slice(&chunk[..n]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let response = match body(root, path) {
        Some((body, kind)) => {
            let kind = declared_type(root, path).unwrap_or_else(|| kind.to_owned());
            let mut head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            head.extend(body);
            head
        }
        None => {
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        }
    };
    let _ = socket.write_all(&response);
}

/** `%xx` escapes, for request paths and the reporter's encoded JSON. */
fn percent_decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/** Content type by extension. */
/** The Content-Type a `<file>.headers` sibling names, which wptserve sends in place of the default:
the encoding suite declares its charset that way, so a page is never served a charset it did not ask for. */
fn declared_type(root: &Path, path: &str) -> Option<String> {
    let text = std::fs::read_to_string(file(root, &format!("{path}.headers"))?).ok()?;
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-type")
            .then(|| value.trim().to_owned())
    })
}

/** The default Content-Type for a file. HTML carries no charset: wptserve sends none, so a page's
own meta (or its `.headers` file) decides how it is decoded. */
fn mime(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html",
        "xhtml" | "xht" => "application/xhtml+xml",
        "xml" => "application/xml",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
        "txt" => "text/plain",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

// ---- discovery ------------------------------------------------------------------------------

/** One test page to load, and why it is skipped if it is. */
struct Case {
    /** URL path under the WPT root, wrapper path and variant query included. */
    path: String,
    /** The `SUITES` entry it is counted in. */
    suite: String,
    /** Some(reason) when the case is not run. */
    skip: Option<&'static str>,
}

/** Every file under a filtered path, including a single selected fixture, skipping helper folders. */
fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    if dir.is_file() {
        files.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(
                name.as_str(),
                "support" | "resources" | "tools" | "reference"
            ) {
                walk(&path, files);
            }
        } else {
            files.push(path);
        }
    }
}

/** The cases one file yields: none when it is not a testharness test. */
fn cases(root: &Path, file: &Path, suite: &str) -> Vec<Case> {
    let Some(path) = file
        .strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
    else {
        return vec![];
    };
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    let source = || String::from_utf8_lossy(&std::fs::read(file).unwrap_or_default()).into_owned();
    let (page, text) = if let Some(stem) = path.strip_suffix(".any.js") {
        let text = source();
        let globals: String = meta(&text)
            .into_iter()
            .filter(|(k, _)| k == "global")
            .map(|(_, v)| v)
            .collect();
        if !globals.is_empty()
            && !globals
                .split(',')
                .any(|g| matches!(g.trim(), "window" | "default"))
        {
            return vec![Case {
                path,
                suite: suite.into(),
                skip: Some("worker-only"),
            }];
        }
        (format!("{stem}.any.html"), text)
    } else if let Some(stem) = path.strip_suffix(".window.js") {
        (format!("{stem}.window.html"), source())
    } else if name.ends_with(".worker.js")
        || name.ends_with(".serviceworker.js")
        || name.ends_with(".sharedworker.js")
    {
        return vec![Case {
            path,
            suite: suite.into(),
            skip: Some("worker"),
        }];
    } else if [".html", ".htm", ".xhtml", ".xht", ".svg"]
        .iter()
        .any(|e| name.ends_with(e))
    {
        let text = source();
        if !text.contains("/resources/testharness.js") {
            return vec![];
        }
        (path.clone(), text)
    } else {
        return vec![];
    };
    let skip = skip_reason(&name, &page);
    let variants: Vec<String> = if path.ends_with(".js") {
        meta(&text)
            .into_iter()
            .filter(|(k, _)| k == "variant")
            .map(|(_, v)| v)
            .collect()
    } else {
        tags_meta_variants(&text)
    };
    let variants = if variants.is_empty() {
        vec![String::new()]
    } else {
        variants
    };
    variants
        .into_iter()
        .map(|v| Case {
            path: format!("{page}{v}"),
            suite: suite.into(),
            skip,
        })
        .collect()
}

/** `<meta name="variant" content="?x">` values in an HTML test. */
fn tags_meta_variants(html: &str) -> Vec<String> {
    html.lines()
        .filter(|l| l.contains("name=\"variant\"") || l.contains("name=variant"))
        .filter_map(|l| {
            let at = l.find("content=")? + "content=".len();
            let rest = l[at..].trim_start_matches(['"', '\'']);
            Some(rest.split(['"', '\'', '>', ' ']).next()?.to_string())
        })
        .filter(|v| v.starts_with('?') || v.starts_with('#'))
        .collect()
}

/** Why a test is not run: it needs a person, TLS, wptserve substitution, or XML. */
fn skip_reason(name: &str, page: &str) -> Option<&'static str> {
    if name.contains("-manual") {
        Some("manual")
    } else if name.contains(".https.") || name.contains(".h2.") {
        Some("https")
    } else if name.contains(".sub.") {
        Some("sub")
    } else if [".xhtml", ".xht", ".svg"].iter().any(|e| page.ends_with(e)) {
        Some("xml")
    } else {
        None
    }
}

// ---- running --------------------------------------------------------------------------------

/** What one page reported. */
struct Outcome {
    /** ok | error | timeout | precondition_failed | incomplete | no-harness | skip | crash. */
    harness: String,
    detail: String,
    /** `(name, status, message)` per subtest. */
    tests: Vec<(String, String, Option<String>)>,
    millis: u128,
}

/** The reporter's results on the current page, if it wrote any. */
fn results(runtime: &mut mars::runtime::Runtime) -> Option<Value> {
    let nodes = agent_call(runtime, "page.nodes", json!({})).ok()?;
    let mut tests = Vec::new();
    let mut status = None;
    for node in nodes["nodes"].as_array()? {
        let Some(payload) = node["attributes"]["data-r"].as_str() else {
            continue;
        };
        let Ok(record) = serde_json::from_str::<Value>(&percent_decode(payload)) else {
            continue;
        };
        match node["tag"].as_str() {
            Some("wpt-result") => tests.push(record),
            Some("wpt-status") => status = Some(record),
            _ => {}
        }
    }
    if tests.is_empty() && status.is_none() {
        return None;
    }
    let field = |name: &str| status.as_ref().map_or(Value::Null, |s| s[name].clone());
    Some(json!({"harness": field("harness"), "message": field("message"), "tests": tests}))
}

/** Load a case and read back its subtests. */
fn run(runtime: &mut mars::runtime::Runtime, origin: &str, case: &Case) -> Outcome {
    let started = Instant::now();
    let navigated = agent_call(
        runtime,
        "page.navigate",
        json!({"url": format!("{origin}/{}", case.path)}),
    );
    let mut found = results(runtime);
    for _ in 0..SETTLE_POLLS {
        if found.as_ref().is_some_and(|r| !r["harness"].is_null()) {
            break;
        }
        match driver::pump(runtime) {
            Ok(true) => eprintln!("native WPT Escape request handled: {}", case.path),
            Err(error) => eprintln!("native WPT input {}: {}", case.path, error.message),
            Ok(false) => {}
        }
        let request = json!({"kind": "evaluate", "expression": "self.__wpt_done", "options": {"await_promise": true}});
        let _ = agent_call(runtime, "page.devtools", json!({"request": request}));
        found = results(runtime);
    }
    let (harness, detail) = match (&found, &navigated) {
        (Some(r), _) => match r["harness"].as_u64() {
            Some(n) => (
                HARNESS.get(n as usize).unwrap_or(&"error").to_string(),
                r["message"].as_str().unwrap_or("").to_string(),
            ),
            None => (
                "incomplete".into(),
                r["message"].as_str().unwrap_or("").to_string(),
            ),
        },
        (None, Err(error)) => ("error".into(), format!("navigate: {error:?}")),
        (None, Ok(page)) => (
            "no-harness".into(),
            page["diagnostics"].to_string().chars().take(600).collect(),
        ),
    };
    let tests = found
        .as_ref()
        .and_then(|r| r["tests"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .map(|t| {
            let status = SUBTEST
                .get(t[1].as_u64().unwrap_or(1) as usize)
                .unwrap_or(&"fail");
            (
                t[0].as_str().unwrap_or("").to_string(),
                status.to_string(),
                t[2].as_str().map(String::from),
            )
        })
        .collect();
    Outcome {
        harness,
        detail,
        tests,
        millis: started.elapsed().as_millis(),
    }
}

/** The `WPT_HARNESS` filter's prefixes, or every suite. */
fn prefixes() -> Vec<String> {
    match std::env::var("WPT_HARNESS") {
        Ok(list) if !list.trim().is_empty() => list
            .split(',')
            .map(|p| p.trim().trim_matches('/').to_string())
            .collect(),
        _ => SUITES.iter().map(|s| s.to_string()).collect(),
    }
}

/** Discover, run in parallel, print per-suite counts and write results-<suite>.json. */
#[test]
#[ignore = "WPT testharness scoreboard; needs ~/Dev/upstream/wpt, run with --ignored"]
fn wpt_testharness() {
    let root = wpt_root();
    let mut all = Vec::new();
    for prefix in prefixes() {
        let suite = SUITES
            .iter()
            .filter(|s| prefix == **s || prefix.starts_with(&format!("{s}/")))
            .max_by_key(|s| s.len())
            .map_or(prefix.clone(), |s| s.to_string());
        let mut files = Vec::new();
        walk(&root.join(&prefix), &mut files);
        files.sort();
        all.extend(files.iter().flat_map(|f| cases(&root, f, &suite)));
    }
    let origin = serve(root.clone());
    let jobs = std::env::var("WPT_JOBS")
        .ok()
        .and_then(|j| j.parse().ok())
        .unwrap_or_else(|| thread::available_parallelism().map_or(4, |n| n.get()));
    let cases = Arc::new(all);
    let next = Arc::new(AtomicUsize::new(0));
    let outcomes: Arc<Mutex<Vec<Option<Outcome>>>> =
        Arc::new(Mutex::new((0..cases.len()).map(|_| None).collect()));
    let started = Instant::now();
    // ponytail: no per-test watchdog; a page that hangs the engine stalls its worker. Add a
    // timeout thread per worker if one ever does.
    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let (cases, next, outcomes, origin) = (
                Arc::clone(&cases),
                Arc::clone(&next),
                Arc::clone(&outcomes),
                origin.clone(),
            );
            thread::spawn(move || {
                let mut engine = None;
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some(case) = cases.get(at) else { break };
                    let outcome = if let Some(reason) = case.skip {
                        Outcome {
                            harness: "skip".into(),
                            detail: reason.into(),
                            tests: vec![],
                            millis: 0,
                        }
                    } else {
                        let current = engine.get_or_insert_with(|| desktop(&origin));
                        match catch_unwind(AssertUnwindSafe(|| run(current, &origin, case))) {
                            Ok(outcome) => outcome,
                            Err(_) => {
                                engine = None;
                                Outcome {
                                    harness: "crash".into(),
                                    detail: "panic".into(),
                                    tests: vec![],
                                    millis: 0,
                                }
                            }
                        }
                    };
                    if outcome.millis > SLOW_MS {
                        eprintln!("slow {}ms {}", outcome.millis, case.path);
                    }
                    outcomes.lock().unwrap()[at] = Some(outcome);
                }
            })
        })
        .collect();
    for worker in workers {
        let _ = worker.join();
    }
    report(&cases, &outcomes.lock().unwrap(), started.elapsed());
}

/** Per-suite counts on stdout and one JSON file per suite. */
fn report(cases: &[Case], outcomes: &[Option<Outcome>], elapsed: Duration) {
    let out = out_dir();
    std::fs::create_dir_all(&out).unwrap();
    let mut by_suite: BTreeMap<&str, (Vec<Value>, BTreeMap<String, usize>)> = BTreeMap::new();
    for (case, outcome) in cases.iter().zip(outcomes) {
        let (tests, counts) = by_suite.entry(&case.suite).or_default();
        let Some(o) = outcome else { continue };
        let key = if o.harness == "skip" {
            format!("skip:{}", o.detail)
        } else {
            format!("harness:{}", o.harness)
        };
        *counts.entry(key).or_default() += 1;
        *counts.entry("files".into()).or_default() += 1;
        for (_, status, _) in &o.tests {
            let bucket = if status == "pass" { "pass" } else { "fail" };
            *counts.entry(bucket.into()).or_default() += 1;
            *counts.entry(format!("subtest:{status}")).or_default() += 1;
        }
        tests.push(json!({
            "test": case.path, "harness": o.harness, "detail": o.detail, "ms": o.millis,
            "subtests": o.tests.iter().map(|(n, s, m)| json!({"name": n, "status": s, "message": m})).collect::<Vec<_>>(),
        }));
    }
    println!("\nsuite          files  pass  fail  rate   harness / skips");
    for (suite, (tests, counts)) in &by_suite {
        let get = |k: &str| counts.get(k).copied().unwrap_or(0);
        let ran = get("pass") + get("fail");
        let rate = if ran == 0 {
            0.0
        } else {
            100.0 * get("pass") as f64 / ran as f64
        };
        let extra: Vec<String> = counts
            .iter()
            .filter(|(k, _)| k.starts_with("harness:") || k.starts_with("skip:"))
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        println!(
            "{suite:<14} {:>5} {:>5} {:>5} {rate:5.1}%  [{}]",
            get("files"),
            get("pass"),
            get("fail"),
            extra.join(" ")
        );
        let body = json!({"suite": suite, "summary": counts, "tests": tests});
        std::fs::write(
            out.join(format!("results-{}.json", suite.replace('/', "-"))),
            serde_json::to_string_pretty(&body).unwrap(),
        )
        .unwrap();
    }
    println!(
        "{} pages in {:.1}s; results in {}",
        cases.len(),
        elapsed.as_secs_f64(),
        out.display()
    );
}
