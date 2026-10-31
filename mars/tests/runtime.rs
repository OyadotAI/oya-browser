/** Real native subprocess tests cover publication, agent tooling and owned lifetimes. */
#[allow(dead_code)]
mod support;
use mars::protocol::Request;
use mars::runtime::{EnginePrograms, Runtime};
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Stdio};
use support::Fixture;

/** All integration calls use the public native operation boundary. */
fn call(runtime: &mut Runtime, op: &str, params: Value) -> mars::protocol::EngineResult<Value> {
    runtime.handle(Request::new("test", op, params))
}
/** Starts the actual three Rust engine processes, not in-process fakes. */
fn runtime() -> Runtime {
    unsafe {
        std::env::set_var("MARS_PROCESS_ISOLATION", "1");
    }
    Runtime::start(EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars")))).unwrap()
}

/** Capability discovery advertises the exact bounded storage host surface. */
#[test]
fn headless_capabilities_advertise_bounded_web_storage() {
    let mut runtime = runtime();
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    assert_eq!(
        capabilities["subsets"]["web_storage"]["localStorage"]["scope"],
        "canonical origin"
    );
    assert_eq!(
        capabilities["subsets"]["web_storage"]["sessionStorage"]["scope"],
        "tab and canonical origin"
    );
    for area in ["localStorage", "sessionStorage"] {
        assert_eq!(
            capabilities["subsets"]["web_storage"][area]["methods"],
            json!(["getItem", "setItem", "removeItem", "clear"])
        );
    }
    assert!(
        !capabilities["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!("storage"))
    );
    assert!(!capabilities.to_string().contains("secret-site-value"));
}

/** Real HTTP, own V9 and native agent mutation all affect observable layout. */
#[test]
fn separate_engines_complete_the_native_agent_flow() {
    let fixture = Fixture::pages();
    let mut runtime = runtime();
    let pids: Vec<_> = runtime
        .process_ids()
        .into_iter()
        .map(|(_, pid)| pid)
        .collect();
    assert_eq!(pids.len(), 3);
    assert!(pids.iter().all(|pid| *pid != std::process::id()));
    let page = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/script", fixture.url)}),
    )
    .unwrap();
    assert_eq!(page["nodes"][0]["text"], "Hello from V9");
    call(&mut runtime, "page.set_text", json!({"page_id":page["page_id"],"node_id":page["nodes"][0]["node_id"],"expected_revision":1,"text":"native"})).unwrap();
    let layout = call(&mut runtime, "page.layout", json!({})).unwrap();
    assert_eq!(layout["rows"][0]["width"], 6);
    assert_eq!(layout["revision"], 2);
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({})).unwrap()["nodes"][0]["text"],
        "native"
    );
    runtime.stop();
    #[cfg(unix)]
    support::assert_gone(&pids);
}

/** Failed candidates never replace a previously committed page or consume its generation. */
#[test]
fn failed_navigation_retains_the_previous_document_atomically() {
    let fixture = Fixture::pages();
    let mut runtime = runtime();
    call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/plain", fixture.url)}),
    )
    .unwrap();
    // A failing script stops only itself and the page still commits; what it did before it
    // threw stays, as in browsers.
    for (path, title) in [("badscript", "candidate"), ("atomic", "changed")] {
        let page = call(
            &mut runtime,
            "page.navigate",
            json!({"url":format!("{}/{path}", fixture.url)}),
        )
        .unwrap();
        assert_eq!(page["degraded"], json!(["scripts"]));
        assert_eq!(page["nodes"][0]["text"], title, "{path}");
    }
    let original = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/plain", fixture.url)}),
    )
    .unwrap();
    // Markup recovers best effort, so a genuine failure is a page past the node budget.
    let error = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/over-budget", fixture.url)}),
    )
    .unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.previous_page_retained);
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({})).unwrap(),
        original
    );
    let next = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/plain", fixture.url)}),
    )
    .unwrap();
    // Four commits came before; the refused candidate consumed no generation.
    assert_eq!(next["page_id"], "page-5");
}

/** Node identities cannot be reused after a successful navigation. */
#[test]
fn stale_page_handles_cannot_mutate_a_replacement_document() {
    let fixture = Fixture::pages();
    let mut runtime = runtime();
    let old = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/plain", fixture.url)}),
    )
    .unwrap();
    let new = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/plain", fixture.url)}),
    )
    .unwrap();
    let error = call(&mut runtime, "page.set_text", json!({"page_id":old["page_id"],"node_id":old["nodes"][0]["node_id"],"expected_revision":1,"text":"wrong"})).unwrap_err();
    assert_eq!(error.code, "invalid_target");
    assert_eq!(call(&mut runtime, "page.inspect", json!({})).unwrap(), new);
}

/** An outdated observation cannot overwrite a newer agent mutation. */
#[test]
fn revisions_prevent_lost_updates() {
    let fixture = Fixture::pages();
    let mut runtime = runtime();
    let page = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/plain", fixture.url)}),
    )
    .unwrap();
    let params = json!({"page_id":page["page_id"],"node_id":page["nodes"][0]["node_id"],"expected_revision":1,"text":"first"});
    call(&mut runtime, "page.set_text", params.clone()).unwrap();
    assert_eq!(
        call(&mut runtime, "page.set_text", params)
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({})).unwrap()["nodes"][0]["text"],
        "first"
    );
}

/** Initial failure cannot manufacture a page or a success response. */
#[test]
fn initial_denied_navigation_leaves_no_page() {
    let mut runtime = runtime();
    let error = call(
        &mut runtime,
        "page.navigate",
        json!({"url":"https://example.com/"}),
    )
    .unwrap_err();
    assert_eq!(error.code, "permission_denied");
    assert!(!error.previous_page_retained);
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({}))
            .unwrap_err()
            .code,
        "no_page"
    );
}

/** CLI replies remain correlated and exclusively JSON even for malformed requests. */
#[test]
fn agent_protocol_recovers_after_correctable_input_errors() {
    let input = b"not json\n{\"version\":1,\"id\":\"extra\",\"op\":\"page.inspect\",\"params\":{},\"extra\":true}\n{\"version\":1,\"id\":\"version\",\"op\":\"page.inspect\",\"params\":{}}\n{\"version\":2,\"id\":\"params\",\"op\":\"page.inspect\",\"params\":{\"extra\":true}}\n{\"version\":2,\"id\":\"unknown\",\"op\":\"Runtime.evaluate\",\"params\":{}}\n{\"version\":2,\"id\":\"caps\",\"op\":\"runtime.capabilities\",\"params\":{}}\n{\"version\":2,\"id\":\"bye\",\"op\":\"runtime.shutdown\",\"params\":{}}\n";
    let output = support::agent(input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 7);
    assert_eq!(replies[0]["id"], Value::Null);
    assert_eq!(replies[1]["id"], "extra");
    assert_eq!(replies[2]["error"]["code"], "invalid_request");
    assert_eq!(replies[3]["error"]["code"], "invalid_request");
    assert_eq!(replies[4]["error"]["code"], "unsupported_operation");
    assert_eq!(replies[5]["result"]["engines"].as_array().unwrap().len(), 3);
    assert_eq!(replies[6]["result"]["stopped"], true);
    #[cfg(unix)]
    support::assert_gone(
        &replies[5]["result"]["engines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|engine| engine["pid"].as_u64().unwrap() as u32)
            .collect::<Vec<_>>(),
    );
}

/** Demo is the documented runnable journey, with actual text and geometry evidence. */
#[test]
fn documented_demo_completes_without_an_existing_browser_engine() {
    let child = Command::new(env!("CARGO_BIN_EXE_mars"))
        .arg("demo")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = support::finish(child);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "Hello from V9",
        "Hello from a native agent",
        "unicode_scalar_cells",
        "Demo complete.",
    ] {
        assert!(text.contains(expected));
    }
}

/** Closing the agent pipe is sufficient to release every child process. */
#[test]
fn stdin_eof_cleans_up_owned_services() {
    let output = support::agent(
        b"{\"version\":2,\"id\":\"caps\",\"op\":\"runtime.capabilities\",\"params\":{}}\n",
    );
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    #[cfg(unix)]
    support::assert_gone(
        &response["result"]["engines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|engine| engine["pid"].as_u64().unwrap() as u32)
            .collect::<Vec<_>>(),
    );
}

/** Output failure releases children even when the library caller retains its Runtime. */
#[test]
fn disconnected_agent_output_retires_children_before_returning() {
    use std::io::{Cursor, Write};
    struct ClosedOutput;
    impl Write for ClosedOutput {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut runtime = runtime();
    let pids: Vec<_> = runtime
        .process_ids()
        .into_iter()
        .map(|(_, pid)| pid)
        .collect();
    let mut input = Cursor::new(
        b"{\"version\":1,\"id\":\"x\",\"op\":\"runtime.capabilities\",\"params\":{}}\n",
    );
    assert!(mars::runtime::run_agent(&mut runtime, &mut input, &mut ClosedOutput).is_err());
    assert!(runtime.process_ids().is_empty());
    #[cfg(unix)]
    support::assert_gone(&pids);
}

/** An idle or active engine discovers parent death through its owned pipe. */
#[cfg(unix)]
#[test]
fn parent_only_termination_and_terminal_interrupt_leave_no_services() {
    for signal in ["parent-kill", "terminal-interrupt", "active-parent-kill"] {
        parent_exit(signal);
    }
}

/** Each coordinator has a separate process group, so tests never signal the test runner. */
#[cfg(unix)]
fn parent_exit(signal: &str) {
    use mars::protocol::read_frame;
    use std::io::{BufReader, Write};
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(env!("CARGO_BIN_EXE_mars"));
    let mut child = command
        .arg("agent")
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(
            b"{\"version\":2,\"id\":\"caps\",\"op\":\"runtime.capabilities\",\"params\":{}}\n",
        )
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let response: Value =
        serde_json::from_slice(&read_frame(&mut output).unwrap().unwrap()).unwrap();
    let pids: Vec<_> = response["result"]["engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|engine| engine["pid"].as_u64().unwrap() as u32)
        .collect();
    let _fixture = if signal == "active-parent-kill" {
        Some(active_request(&mut child))
    } else {
        None
    };
    if signal == "terminal-interrupt" {
        assert!(
            Command::new("/bin/kill")
                .args(["-INT", &format!("-{}", child.id())])
                .status()
                .unwrap()
                .success()
        );
    } else {
        child.kill().unwrap();
    }
    assert!(!support::finish(child).status.success());
    support::assert_gone(&pids);
}

/** Waiting for accept ensures the parent dies while the network service owns work. */
#[cfg(unix)]
fn active_request(child: &mut std::process::Child) -> Fixture {
    use std::io::Write;
    use std::sync::atomic::Ordering;
    use std::time::Instant;
    let fixture = Fixture::slow();
    let request = json!({"version":2,"id":"active","op":"page.navigate","params":{"url":format!("{}/",fixture.url)}});
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&mars::protocol::encode(&request).unwrap())
        .unwrap();
    let deadline = Instant::now() + support::COMPLETION;
    while fixture.requests.load(Ordering::Relaxed) == 0 {
        assert!(Instant::now() < deadline, "fixture was never requested");
        std::thread::sleep(support::POLL);
    }
    fixture
}
