/** WPT network scoreboard for Relay: runs WPT's own Python server (wptserve, from the upstream
checkout's vendored tools, no virtualenv) on loopback, navigates the full Mars runtime to each
testharness test under fetch/, cors/, cookies/, mimesniff/ and xhr/, and counts the subtests the
same way tests/wpt_harness.rs does (a stand-in testharnessreport.js, mounted through a wptserve
alias, writes the results into a `data-r` attribute that page.nodes reads back).

Host names: wptserve's browser_host is set to the literal 127.0.0.1, because Relay admits a
loopback destination only when its URL host is a literal loopback IP and its exact origin is
granted (`fixture_granted`, the same allowance tests/support's Fixture servers use). The runtime is
granted the two HTTP origins, so cross-origin tests that use HTTP_PORT2 work; tests that need
the www1 and www2 subdomains or not-web-platform.test fail to connect, and no /etc/hosts edit or
resolver override is made. HTTPS and HTTP/2 tests are skipped: Relay trusts only the Mozilla roots
and WPT's certificates are for web-platform.test. Ignored by default; run it with

    WPT_NET=fetch/api/basic cargo test --offline --test wpt_network -- --ignored --nocapture

`WPT_NET` is a comma-separated list of path prefixes under the WPT root (`xhr`, or narrower, like
`fetch/api/cors`); unset means every suite in `SUITES`. `WPT_ROOT` overrides the checkout,
`WPT_OUT` the results folder, `WPT_JOBS` the thread count, `WPT_POLLS` the page.wait rounds. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::net::{TcpListener, TcpStream};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use support::agent_call;

/** The suites the scoreboard covers when `WPT_NET` names none. */
const SUITES: &[&str] = &["fetch", "cors", "cookies", "mimesniff", "xhr"];
/** How long wptserve may take to start answering. */
const STARTUP: Duration = Duration::from_secs(60);
/** Default page.wait rounds of 2s after navigation: testharness.js's own 10s default timeout. */
const SETTLE_POLLS: usize = 5;
/** The reporter that stands in for testharnessreport.js (same as tests/wpt_harness.rs). */
const REPORTER: &str = r#"(function () {
  var results = [];
  function message(m) { return m == null ? null : String(m).slice(0, 300); }
  function put(harness, hmessage) {
    var el = document.getElementById('__wpt_results');
    if (!el) {
      el = document.createElement('wpt-results');
      el.id = '__wpt_results';
      (document.body || document.documentElement).appendChild(el);
    }
    el.setAttribute('data-r', encodeURIComponent(JSON.stringify(
      {harness: harness, message: message(hmessage), tests: results})));
  }
  add_result_callback(function (t) {
    results.push([String(t.name), t.status, message(t.message)]);
    try { put(null, null); } catch (e) {}
  });
  add_completion_callback(function (tests, status) {
    results = tests.map(function (t) { return [String(t.name), t.status, message(t.message)]; });
    put(status.status, status.message);
  });
})();
"#;
/** Starts wptserve from the checkout with its vendored packages on the path. The main guard
matters: wptserve's stash uses multiprocessing spawn, which re-imports this file. */
const LAUNCHER: &str = r#"import os, sys
root = os.environ["WPT_UPSTREAM"]
sys.path.insert(0, os.path.join(root, "tools"))
import localpaths  # noqa: F401
if __name__ == "__main__":
    os.chdir(root)
    from serve import serve
    sys.argv = ["serve", "--config", os.environ["WPT_CONFIG"]]
    sys.exit(serve.main())
"#;
/** testharness.js subtest statuses by number. */
const SUBTEST: &[&str] = &["pass", "fail", "timeout", "notrun", "precondition_failed"];
/** testharness.js harness statuses by number. */
const HARNESS: &[&str] = &["ok", "error", "timeout", "precondition_failed"];

/** The upstream WPT checkout. */
fn wpt_root() -> PathBuf {
    std::env::var_os("WPT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap()).join("Dev/upstream/wpt"))
}

/** Where results-<suite>.json, the wptserve config and its log land. */
fn out_dir() -> PathBuf {
    std::env::var_os("WPT_OUT").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("/private/tmp/claude-501/-Users-mk-Dev-oya-agentchrome/0608a5c7-c382-49f9-b32d-bb2fdc1f40c0/scratchpad/conf/network")
    })
}

// ---- wptserve -------------------------------------------------------------------------------

/** A running wptserve, killed when dropped. */
struct Wptserve {
    child: Child,
    /** The two HTTP origins, the ones the runtime is granted. */
    origins: [String; 2],
}

impl Drop for Wptserve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/** A free loopback port (released at once; wptserve binds it a moment later). */
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/** Write the config, reporter and launcher into `out`, start wptserve, wait until it answers. */
fn wptserve(root: &Path, out: &Path) -> Wptserve {
    let ports: Vec<u16> = (0..5).map(|_| free_port()).collect();
    let reporter = out.join("testharnessreport.js");
    std::fs::write(&reporter, REPORTER).unwrap();
    let config = json!({
        "browser_host": "127.0.0.1", "server_host": "127.0.0.1", "check_subdomains": false,
        "ports": {"http": [ports[0], ports[1]], "https": [ports[2], ports[3]], "h2": [ports[4]],
                  "ws": [], "wss": [], "webtransport-h3": []},
        "ssl": {"type": "pregenerated"},
        "aliases": [{"url-path": "/resources/testharnessreport.js", "local-dir": out}],
    });
    let config_path = out.join("wpt-config.json");
    std::fs::write(&config_path, config.to_string()).unwrap();
    let launcher = out.join("wptserve-launch.py");
    std::fs::write(&launcher, LAUNCHER).unwrap();
    let log = std::fs::File::create(out.join("wptserve.log")).unwrap();
    let child = Command::new("python3")
        .arg("-I")
        .arg(&launcher)
        .env("WPT_UPSTREAM", root)
        .env("WPT_CONFIG", &config_path)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .expect("python3 must be on PATH to run wptserve");
    let server = Wptserve {
        child,
        origins: [0, 1].map(|i| format!("http://127.0.0.1:{}", ports[i])),
    };
    let deadline = Instant::now() + STARTUP;
    while ports[..2]
        .iter()
        .any(|p| TcpStream::connect(("127.0.0.1", *p)).is_err())
    {
        assert!(
            Instant::now() < deadline,
            "wptserve did not start; see {}",
            out.join("wptserve.log").display()
        );
        thread::sleep(Duration::from_millis(200));
    }
    server
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

/** Every file under `dir`, recursively, skipping helper folders. */
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
                "support" | "resources" | "tools" | "reference"
            ) {
                walk(&path, files);
            }
        } else {
            files.push(path);
        }
    }
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

/** The cases one file yields: none when it is not a testharness test. wptserve generates the
`.any.html` and `.window.html` wrappers and does `.sub.` substitution itself. */
fn cases(root: &Path, file: &Path, suite: &str) -> Vec<Case> {
    let Some(path) = file
        .strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
    else {
        return vec![];
    };
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    let text = || String::from_utf8_lossy(&std::fs::read(file).unwrap_or_default()).into_owned();
    let one = |path: String, skip| {
        vec![Case {
            path,
            suite: suite.into(),
            skip,
        }]
    };
    let (page, source) = if let Some(stem) = path.strip_suffix(".any.js") {
        let source = text();
        let globals: String = meta(&source)
            .into_iter()
            .filter(|(k, _)| k == "global")
            .map(|(_, v)| v)
            .collect();
        if !globals.is_empty()
            && !globals
                .split(',')
                .any(|g| matches!(g.trim(), "window" | "default"))
        {
            return one(path, Some("worker-only"));
        }
        (format!("{stem}.any.html"), source)
    } else if let Some(stem) = path.strip_suffix(".window.js") {
        (format!("{stem}.window.html"), text())
    } else if name.ends_with(".worker.js")
        || name.ends_with(".serviceworker.js")
        || name.ends_with(".sharedworker.js")
    {
        return one(path, Some("worker"));
    } else if name.ends_with(".html") || name.ends_with(".htm") {
        let source = text();
        if !source.contains("/resources/testharness.js") {
            return vec![];
        }
        (path.clone(), source)
    } else {
        return vec![];
    };
    let skip = if name.contains("-manual") {
        Some("manual")
    } else if name.contains(".https.") || name.contains(".h2.") {
        Some("https")
    } else {
        None
    };
    let mut variants: Vec<String> = if path.ends_with(".js") {
        meta(&source)
            .into_iter()
            .filter(|(k, _)| k == "variant")
            .map(|(_, v)| v)
            .collect()
    } else {
        source
            .lines()
            .filter(|l| l.contains("name=\"variant\"") || l.contains("name=variant"))
            .filter_map(|l| {
                let rest =
                    l[l.find("content=")? + "content=".len()..].trim_start_matches(['"', '\'']);
                Some(rest.split(['"', '\'', '>', ' ']).next()?.to_string())
            })
            .filter(|v| v.starts_with('?') || v.starts_with('#'))
            .collect()
    };
    if variants.is_empty() {
        variants.push(String::new());
    }
    variants
        .into_iter()
        .flat_map(|v| one(format!("{page}{v}"), skip))
        .collect()
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

/** `%xx` escapes in the reporter's encoded JSON. */
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
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

/** The reporter's results on the current page, if it wrote any. */
fn results(runtime: &mut mars::runtime::Runtime) -> Option<Value> {
    let nodes = agent_call(runtime, "page.nodes", json!({})).ok()?;
    let node = nodes["nodes"]
        .as_array()?
        .iter()
        .find(|n| n["attributes"]["id"] == "__wpt_results")?;
    serde_json::from_str(&percent_decode(node["attributes"]["data-r"].as_str()?)).ok()
}

/** Load a case and read back its subtests. */
fn run(runtime: &mut mars::runtime::Runtime, origin: &str, case: &Case, polls: usize) -> Outcome {
    let started = Instant::now();
    let navigated = agent_call(
        runtime,
        "page.navigate",
        json!({"url": format!("{origin}/{}", case.path)}),
    );
    let mut found = results(runtime);
    for _ in 0..polls {
        if found.as_ref().is_some_and(|r| !r["harness"].is_null()) {
            break;
        }
        let _ = agent_call(
            runtime,
            "page.wait",
            json!({"text": "\u{1}", "timeout_ms": 2000}),
        );
        found = results(runtime);
    }
    let (harness, detail) = match (&found, &navigated) {
        (Some(r), _) => match r["harness"].as_u64() {
            Some(n) => (
                HARNESS.get(n as usize).unwrap_or(&"error").to_string(),
                r["message"].as_str().unwrap_or("").to_string(),
            ),
            None => ("incomplete".into(), String::new()),
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

/** A desktop runtime granted both wptserve HTTP origins. */
fn runtime(origins: &[String]) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins.to_vec(),
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** The `WPT_NET` filter's prefixes, or every suite. */
fn prefixes() -> Vec<String> {
    match std::env::var("WPT_NET") {
        Ok(list) if !list.trim().is_empty() => list
            .split(',')
            .map(|p| p.trim().trim_matches('/').to_string())
            .collect(),
        _ => SUITES.iter().map(|s| s.to_string()).collect(),
    }
}

/** Start wptserve, discover, run in parallel, print per-suite counts, write results JSON. */
#[test]
#[ignore = "WPT network scoreboard; needs ~/Dev/upstream/wpt and python3, run with --ignored"]
fn wpt_network() {
    let (root, out) = (wpt_root(), out_dir());
    std::fs::create_dir_all(&out).unwrap();
    let mut all = Vec::new();
    for prefix in prefixes() {
        let suite = prefix.split('/').next().unwrap().to_string();
        let mut files = Vec::new();
        walk(&root.join(&prefix), &mut files);
        files.sort();
        all.extend(files.iter().flat_map(|f| cases(&root, f, &suite)));
    }
    let server = wptserve(&root, &out);
    let origins = server.origins.to_vec();
    let jobs = std::env::var("WPT_JOBS")
        .ok()
        .and_then(|j| j.parse().ok())
        .unwrap_or_else(|| thread::available_parallelism().map_or(4, |n| n.get()));
    let polls = std::env::var("WPT_POLLS")
        .ok()
        .and_then(|j| j.parse().ok())
        .unwrap_or(SETTLE_POLLS);
    let cases = Arc::new(all);
    let next = Arc::new(AtomicUsize::new(0));
    let outcomes: Arc<Mutex<Vec<Option<Outcome>>>> =
        Arc::new(Mutex::new((0..cases.len()).map(|_| None).collect()));
    let started = Instant::now();
    // ponytail: no per-test watchdog; a page that hangs the engine stalls its worker. Add a
    // timeout thread per worker if one ever does.
    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let (cases, next, outcomes, origins) = (
                Arc::clone(&cases),
                Arc::clone(&next),
                Arc::clone(&outcomes),
                origins.clone(),
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
                        let current = engine.get_or_insert_with(|| runtime(&origins));
                        match catch_unwind(AssertUnwindSafe(|| {
                            run(current, &origins[0], case, polls)
                        })) {
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
                    outcomes.lock().unwrap()[at] = Some(outcome);
                }
            })
        })
        .collect();
    for worker in workers {
        let _ = worker.join();
    }
    drop(server);
    report(&cases, &outcomes.lock().unwrap(), started.elapsed(), &out);
}

/** Per-suite counts on stdout and one JSON file per suite. */
fn report(cases: &[Case], outcomes: &[Option<Outcome>], elapsed: Duration, out: &Path) {
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
            *counts
                .entry(if status == "pass" { "pass" } else { "fail" }.into())
                .or_default() += 1;
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
            out.join(format!("results-{suite}.json")),
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
