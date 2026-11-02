/** WPT reftest scoreboard: serves Blink's web-platform tests from disk over loopback, renders each
test and its reference through a real navigation at 800x600, rasterizes both with the windowless
painter (page.screenshot) and compares the pixels exactly. Ignored by default; run it with

    WPT=css-flexbox cargo test --offline --test wpt_runner -- --ignored --nocapture

`WPT` is a comma-separated list of path prefixes under `css/` (a suite like `css-flexbox`, or a
narrower prefix like `css-flexbox/align-items`); unset means every suite in `SUITES`.
`WPT_ROOT` overrides the checkout, `WPT_OUT` the results folder, `WPT_JOBS` the thread count. */
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

/** The suites the scoreboard covers when `WPT` names none. */
const SUITES: &[&str] = &[
    "css-flexbox",
    "css-grid",
    "css-tables",
    "CSS2",
    "css-sizing",
    "css-position",
    "css-overflow",
    "css-text",
    "css-display",
    "css-box",
    "css-backgrounds",
    "css-inline",
    "selectors",
    "css-cascade",
    "css-values",
    "css-nesting",
    "css-multicol",
    "css-writing-modes",
];
/** The WPT default reftest viewport. */
const VIEWPORT: (f64, f64) = (800.0, 600.0);
/** An empty page the server synthesizes, to flag passes where both sides painted nothing. */
const BLANK: &str = "/__wpt_blank.html";
/** A loopback client that stalls never pins a server thread. */
const SOCKET_TIMEOUT: Duration = Duration::from_secs(5);

/** The Blink checkout's WPT root, the document root the server serves. */
fn wpt_root() -> PathBuf {
    std::env::var_os("WPT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap())
                .join("Dev/upstream/chromium/third_party/blink/web_tests/external/wpt")
        })
}

/** Where results-<suite>.json lands. */
fn out_dir() -> PathBuf {
    std::env::var_os("WPT_OUT").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("/private/tmp/claude-501/-Users-mk-Dev-oya-agentchrome/0608a5c7-c382-49f9-b32d-bb2fdc1f40c0/scratchpad/wpt")
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

/** Answer one request with the file under `root`, or 404; never anything outside `root`. */
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
    let file = root
        .join(percent_decode(path).trim_start_matches('/'))
        .canonicalize()
        .ok()
        .filter(|file| file.starts_with(root) && file.is_file());
    let file = if path == BLANK { None } else { file };
    let blank = (path == BLANK).then(|| (b"<!doctype html>".to_vec(), PathBuf::from("blank.html")));
    let response = match blank
        .or_else(|| file.and_then(|file| Some((std::fs::read(&file).ok()?, file))))
    {
        Some((body, file)) => {
            let mut head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                mime(&file),
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

/** `%xx` escapes in a request path, enough for WPT file names. */
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

/** Content type by extension, as wptserve sends it: XHTML is application/xhtml+xml, so Mars
reads `.xht` tests with its XML parser as Blink does. */
fn mime(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "xht" | "xhtml" => "application/xhtml+xml; charset=utf-8",
        "css" => "text/css",
        "js" => "text/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}

// ---- discovery ------------------------------------------------------------------------------

/** One test file, its references, and why it is skipped if it is. */
struct Case {
    /** Path under the WPT root, e.g. `css/css-flexbox/align-items-001.html`. */
    path: String,
    /** First component under css/, the suite it is counted in. */
    suite: String,
    /** `(true = match | false = mismatch, reference path under the root)`. */
    refs: Vec<(bool, String)>,
    /** Some(reason) when the case is not run. */
    skip: Option<&'static str>,
}

/** Every file under `dir`, recursively, skipping support and reference folders. */
fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(
                name.as_str(),
                "support" | "reference" | "resources" | "tools"
            ) {
                walk(&path, files);
            }
        } else if [".html", ".htm", ".xht", ".xhtml"]
            .iter()
            .any(|e| name.ends_with(e))
            && !name.contains("-ref.")
            && !name.contains("-notref.")
        {
            files.push(path);
        }
    }
}

/** The start tags named `tag` in `html`, each as its attribute map (names lowercased). */
fn tags(html: &str, tag: &str) -> Vec<BTreeMap<String, String>> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find(&open).map(|i| i + from) {
        let start = at + open.len();
        from = start;
        if !lower[start..].starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/') {
            continue;
        }
        let end = lower[start..].find('>').map_or(lower.len(), |i| i + start);
        found.push(attributes(&html[start..end]));
        from = end;
    }
    found
}

/** A tolerant attribute parser: name, optional `=`, quoted or bare value. */
fn attributes(text: &str) -> BTreeMap<String, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut map = BTreeMap::new();
    let mut at = 0;
    let take = |at: &mut usize, stop: &dyn Fn(char) -> bool| {
        let start = *at;
        while *at < chars.len() && !stop(chars[*at]) {
            *at += 1;
        }
        chars[start..*at].iter().collect::<String>()
    };
    while at < chars.len() {
        if chars[at].is_whitespace() || chars[at] == '/' {
            at += 1;
            continue;
        }
        let name =
            take(&mut at, &|c| c.is_whitespace() || c == '=' || c == '/').to_ascii_lowercase();
        let mut value = String::new();
        if chars.get(at) == Some(&'=') {
            at += 1;
            match chars.get(at) {
                Some(&quote) if quote == '"' || quote == '\'' => {
                    at += 1;
                    value = take(&mut at, &|c| c == quote);
                    at += 1;
                }
                _ => value = take(&mut at, &|c| c.is_whitespace()),
            }
        }
        map.entry(name).or_insert(value);
    }
    map
}

/** Read one test file into a Case, or None when it is not a reftest. */
fn case(root: &Path, file: &Path) -> Option<Case> {
    let html = String::from_utf8_lossy(&std::fs::read(file).ok()?).into_owned();
    let path = file
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let dir = Path::new(&path).parent()?.to_path_buf();
    let refs: Vec<(bool, String)> = tags(&html, "link")
        .into_iter()
        .filter_map(|link| {
            let rel = link.get("rel")?.to_ascii_lowercase();
            let matching = match rel.trim() {
                "match" => true,
                "mismatch" => false,
                _ => return None,
            };
            let href = link.get("href")?.split(['?', '#']).next()?.to_string();
            let target = match href.strip_prefix('/') {
                Some(absolute) => PathBuf::from(absolute),
                None => dir.join(&href),
            };
            Some((matching, normalize(&target)))
        })
        .collect();
    if refs.is_empty() {
        return None;
    }
    let suite = path.strip_prefix("css/")?.split('/').next()?.to_string();
    let skip = skip_reason(root, &path, &html, &refs);
    Some(Case {
        path,
        suite,
        refs,
        skip,
    })
}

/** Resolve `.` and `..` in a relative path without touching the filesystem. */
fn normalize(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for part in path.components() {
        match part.as_os_str().to_string_lossy().as_ref() {
            "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other.to_string()),
        }
    }
    parts.join("/")
}

/** Why a reftest is not run: only what Mars cannot do (paged media, a person at the keyboard)
or a reference that is not in the checkout. Script and reftest-wait tests run, as navigation runs
the page's scripts through V9 and `settle` waits out reftest-wait. */
fn skip_reason(
    root: &Path,
    path: &str,
    html: &str,
    refs: &[(bool, String)],
) -> Option<&'static str> {
    let flags: String = tags(html, "meta")
        .into_iter()
        .filter(|meta| {
            meta.get("name")
                .is_some_and(|n| n.eq_ignore_ascii_case("flags"))
        })
        .filter_map(|meta| meta.get("content").cloned())
        .collect::<Vec<_>>()
        .join(" ");
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.contains("-manual") {
        return Some("manual");
    }
    if flags.contains("interact") {
        return Some("interact");
    }
    if name.contains("-print") || path.contains("/print/") || flags.contains("paged") {
        return Some("print");
    }
    if refs.iter().any(|(_, r)| !root.join(r).is_file()) {
        return Some("missing-reference");
    }
    None
}

// ---- running --------------------------------------------------------------------------------

/** The outcome of one case. */
struct Outcome {
    status: &'static str,
    detail: String,
    millis: u128,
}

/** Real time a reftest-wait page gets to drop the class before it counts as a timeout. */
const WAIT_TIMEOUT: Duration = Duration::from_secs(6);
/** How often the runner pumps the page's timers while it waits, the coordinator's tick. */
const WAIT_POLL: Duration = Duration::from_millis(100);

/** Whether `<html>`'s class list still holds `reftest-wait`. */
fn waiting(runtime: &mut mars::runtime::Runtime) -> Result<bool, String> {
    let nodes =
        agent_call(runtime, "page.nodes", json!({})).map_err(|e| format!("nodes: {e:?}"))?;
    Ok(nodes["html_attributes"]["class"]
        .as_str()
        .is_some_and(|class| class.split_ascii_whitespace().any(|c| c == "reftest-wait")))
}

/** Run the page's timers and requests on the coordinator's schedule, as the desktop broker
does, until `<html>` drops `reftest-wait` (WPT's signal that the page is ready to compare) or
WAIT_TIMEOUT passes. Pages that never had the class return at once. */
fn settle(runtime: &mut mars::runtime::Runtime) -> Result<(), String> {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    while waiting(runtime)? {
        if Instant::now() >= deadline {
            return Err("reftest-wait timeout".into());
        }
        thread::sleep(WAIT_POLL);
        runtime.run_page_scripts();
    }
    Ok(())
}

/** Navigate to `url`, let it settle, and return the viewport screenshot's PNG (base64) and size. */
fn shoot(runtime: &mut mars::runtime::Runtime, url: &str) -> Result<String, String> {
    agent_call(runtime, "page.navigate", json!({"url": url}))
        .map_err(|e| format!("navigate: {e:?}"))?;
    settle(runtime)?;
    let shot = agent_call(runtime, "page.screenshot", json!({}))
        .map_err(|e| format!("screenshot: {e:?}"))?;
    Ok(format!(
        "{}x{}:{}",
        shot["width"],
        shot["height"],
        shot["png_base64"].as_str().unwrap_or("")
    ))
}

/** Render a case's test and references and compare. Multiple match references are
alternatives (any one passes); every mismatch reference must differ. */
fn run(runtime: &mut mars::runtime::Runtime, origin: &str, case: &Case, blank: &str) -> Outcome {
    let started = Instant::now();
    let result = (|| -> Result<(bool, String), String> {
        let test = shoot(runtime, &format!("{origin}/{}", case.path))?;
        let mut any_match = None;
        let mut nearest = usize::MAX;
        for (matching, reference) in &case.refs {
            let shot = shoot(runtime, &format!("{origin}/{reference}"))?;
            let same = shot == test;
            if *matching {
                any_match = Some(any_match.unwrap_or(false) || same);
                if !same {
                    nearest = nearest.min(pixel_diff(&test, &shot));
                }
            } else if same {
                return Ok((false, format!("matches mismatch reference {reference}")));
            }
        }
        let note = match (test == blank, any_match) {
            (true, _) => "blank".to_string(),
            (false, Some(false)) => format!("{nearest} px differ"),
            _ => String::new(),
        };
        Ok((any_match.unwrap_or(true), note))
    })();
    let (status, detail) = match result {
        Ok((true, note)) => ("pass", note),
        Ok((false, detail)) => ("fail", detail),
        Err(error) if error.contains("reftest-wait timeout") => ("fail", error),
        Err(error) => ("error", error),
    };
    Outcome {
        status,
        detail,
        millis: started.elapsed().as_millis(),
    }
}

/** How many pixels two `WxH:base64png` shots differ in, so near misses (antialiasing, a
sub-pixel offset) can be told from layout failures; usize::MAX when sizes differ. */
fn pixel_diff(a: &str, b: &str) -> usize {
    let (Some(a), Some(b)) = (pixels(a), pixels(b)) else {
        return usize::MAX;
    };
    if a.len() != b.len() {
        return usize::MAX;
    }
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
}

/** A shot's decoded RGBA pixels. */
fn pixels(shot: &str) -> Option<Vec<u8>> {
    let encoded = shot.split_once(':')?.1;
    let mut bytes = Vec::with_capacity(encoded.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in encoded.bytes().filter(|c| *c != b'=') {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
        }
    }
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .ok()?;
    let mut out = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut out).ok()?;
    out.truncate(info.buffer_size());
    let channels = info.color_type.samples();
    Some(
        out.chunks(channels)
            .flat_map(|p| [p[0], p[1], p[2], 0])
            .collect(),
    )
}

/** A runtime at the WPT viewport with only the server's origin granted. */
fn runtime(origin: &str) -> mars::runtime::Runtime {
    let mut runtime = desktop(origin);
    agent_call(
        &mut runtime,
        "page.emulate",
        json!({"viewport": {"width": VIEWPORT.0, "height": VIEWPORT.1}}),
    )
    .unwrap();
    runtime
}

/** The `WPT` filter's prefixes, or every suite. */
fn prefixes() -> Vec<String> {
    match std::env::var("WPT") {
        Ok(list) if !list.trim().is_empty() => list
            .split(',')
            .map(|p| p.trim().trim_start_matches("css/").to_string())
            .collect(),
        _ => SUITES.iter().map(|s| s.to_string()).collect(),
    }
}

/** Discover, run in parallel, print per-suite counts and write results-<suite>.json. */
#[test]
#[ignore = "WPT scoreboard; needs the Blink checkout, run with --ignored"]
fn wpt_reftests() {
    let root = wpt_root();
    let prefixes = prefixes();
    let mut files = Vec::new();
    let suites: std::collections::BTreeSet<String> = prefixes
        .iter()
        .map(|p| p.split('/').next().unwrap().to_string())
        .collect();
    for suite in &suites {
        walk(&root.join("css").join(suite), &mut files);
    }
    let cases: Vec<Case> = files
        .iter()
        .filter_map(|file| case(&root, file))
        .filter(|case| {
            prefixes
                .iter()
                .any(|p| case.path["css/".len()..].starts_with(p.as_str()))
        })
        .collect();
    let origin = serve(root.clone());
    let jobs = std::env::var("WPT_JOBS")
        .ok()
        .and_then(|j| j.parse().ok())
        .unwrap_or_else(|| thread::available_parallelism().map_or(4, |n| n.get()));
    let cases = Arc::new(cases);
    let next = Arc::new(AtomicUsize::new(0));
    let outcomes: Arc<Mutex<Vec<Option<Outcome>>>> =
        Arc::new(Mutex::new((0..cases.len()).map(|_| None).collect()));
    let started = Instant::now();
    // ponytail: no per-test watchdog; a test that hangs the engine stalls its worker. Add a
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
                let mut blank = None;
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some(case) = cases.get(at) else { break };
                    let outcome = if let Some(reason) = case.skip {
                        Outcome {
                            status: "skip",
                            detail: reason.to_string(),
                            millis: 0,
                        }
                    } else {
                        let current = engine.get_or_insert_with(|| runtime(&origin));
                        let blank = blank.get_or_insert_with(|| {
                            shoot(current, &format!("{origin}{BLANK}")).unwrap_or_default()
                        });
                        match catch_unwind(AssertUnwindSafe(|| run(current, &origin, case, blank)))
                        {
                            Ok(outcome) => outcome,
                            Err(_) => {
                                engine = None;
                                Outcome {
                                    status: "error",
                                    detail: "panic".into(),
                                    millis: 0,
                                }
                            }
                        }
                    };
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

/** Debug one case: `WPT_PROBE=css/x/a.html` writes the test's and each reference's screenshot
(base64 PNG, `*.b64`) and point layout (`*.json`) to `WPT_OUT/probe`, so a failure can be read
instead of guessed. */
#[test]
#[ignore = "WPT debugging aid; run with WPT_PROBE=<path> --ignored"]
fn wpt_probe() {
    let Ok(path) = std::env::var("WPT_PROBE") else {
        return;
    };
    let root = wpt_root();
    let case = case(&root, &root.join(&path)).expect("a reftest");
    let origin = serve(root);
    let mut runtime = runtime(&origin);
    let out = out_dir().join("probe");
    std::fs::create_dir_all(&out).unwrap();
    let mut pages = vec![("test".to_string(), case.path.clone())];
    for (at, (_, reference)) in case.refs.iter().enumerate() {
        pages.push((format!("ref{at}"), reference.clone()));
    }
    for (name, page) in pages {
        let shot = shoot(&mut runtime, &format!("{origin}/{page}")).unwrap();
        let png = shot.split_once(':').map_or("", |(_, png)| png);
        std::fs::write(out.join(format!("{name}.b64")), png).unwrap();
        let layout = agent_call(&mut runtime, "page.layout", json!({})).unwrap();
        let text = serde_json::to_string_pretty(&layout).unwrap();
        std::fs::write(out.join(format!("{name}.json")), text).unwrap();
        println!("{name}: {page}");
    }
}

/** Per-suite counts on stdout and one JSON file per suite. */
fn report(cases: &[Case], outcomes: &[Option<Outcome>], elapsed: Duration) {
    let out = out_dir();
    std::fs::create_dir_all(&out).unwrap();
    let mut by_suite: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    let mut counts: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for (case, outcome) in cases.iter().zip(outcomes) {
        let outcome = outcome.as_ref().map_or(("error", "not run", 0), |o| {
            (o.status, o.detail.as_str(), o.millis)
        });
        *counts
            .entry(&case.suite)
            .or_default()
            .entry(outcome.0)
            .or_default() += 1;
        if outcome.0 == "pass" && outcome.1 == "blank" {
            *counts
                .entry(&case.suite)
                .or_default()
                .entry("pass:blank")
                .or_default() += 1;
        }
        if outcome.0 == "skip" {
            *counts
                .entry(&case.suite)
                .or_default()
                .entry(leak(format!("skip:{}", outcome.1)))
                .or_default() += 1;
        }
        by_suite.entry(&case.suite).or_default().push(json!({
            "test": case.path, "status": outcome.0, "detail": outcome.1, "ms": outcome.2,
            "refs": case.refs.iter().map(|(m, r)| json!({"rel": if *m {"match"} else {"mismatch"}, "href": r})).collect::<Vec<_>>(),
        }));
    }
    println!("\nsuite                pass blank  fail error  skip   rate(of run)");
    for (suite, tests) in &by_suite {
        let c = &counts[suite];
        let get = |k: &str| c.get(k).copied().unwrap_or(0);
        let ran = get("pass") + get("fail") + get("error");
        let rate = if ran == 0 {
            0.0
        } else {
            100.0 * get("pass") as f64 / ran as f64
        };
        let skips: Vec<String> = c
            .iter()
            .filter(|(k, _)| k.starts_with("skip:"))
            .map(|(k, v)| format!("{}={v}", &k[5..]))
            .collect();
        println!(
            "{suite:<20} {:>5} {:>5} {:>5} {:>5} {:>5}   {rate:5.1}%   [{}]",
            get("pass"),
            get("pass:blank"),
            get("fail"),
            get("error"),
            get("skip"),
            skips.join(" ")
        );
        let summary: BTreeMap<&str, usize> = c.iter().map(|(k, v)| (*k, *v)).collect();
        let body = json!({"suite": suite, "summary": summary, "tests": tests});
        std::fs::write(
            out.join(format!("results-{suite}.json")),
            serde_json::to_string_pretty(&body).unwrap(),
        )
        .unwrap();
    }
    let total = |k: &str| {
        counts
            .values()
            .map(|c| c.get(k).copied().unwrap_or(0))
            .sum::<usize>()
    };
    let ran = total("pass") + total("fail") + total("error");
    println!(
        "{:<20} {:>5} {:>5} {:>5} {:>5} {:>5}   {:5.1}%",
        "TOTAL",
        total("pass"),
        total("pass:blank"),
        total("fail"),
        total("error"),
        total("skip"),
        100.0 * total("pass") as f64 / ran.max(1) as f64
    );
    println!(
        "{} cases in {:.1}s; results in {}",
        cases.len(),
        elapsed.as_secs_f64(),
        out.display()
    );
}

/** Counter keys live for the report; a handful of skip reasons is a few bytes. */
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/** Only what Mars cannot do is skipped: script and reftest-wait cases run; manual, interact and
print do not, and each is counted under its own reason. */
#[test]
fn skips_only_what_mars_cannot_do() {
    let root = std::env::temp_dir().join(format!("wpt-skips-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a-ref.html"), "<script>1</script>").unwrap();
    let refs = [(true, "a-ref.html".to_string())];
    let reason = |path: &str, html: &str| skip_reason(&root, path, html, &refs);
    assert_eq!(reason("css/x/a.html", "<script>x()</script>"), None);
    assert_eq!(reason("css/x/a.html", "<html class='reftest-wait'>"), None);
    assert_eq!(reason("css/x/a-visual.html", ""), None);
    assert_eq!(reason("css/x/a-manual.html", ""), Some("manual"));
    let interact = "<meta name=flags content='interact'>";
    assert_eq!(reason("css/x/a.html", interact), Some("interact"));
    assert_eq!(reason("css/x/a-print.html", ""), Some("print"));
    let missing = [(true, "gone-ref.html".to_string())];
    assert_eq!(
        skip_reason(&root, "css/x/a.html", "", &missing),
        Some("missing-reference")
    );
    let _ = std::fs::remove_dir_all(&root);
}
