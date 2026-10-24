/*! Opt-in real macOS-window checks use Mars's own protocol and bitmap capture only. */
#![cfg(all(target_os = "macos", feature = "test-support"))]
#[path = "window/banner.rs"]
mod banner;
#[path = "window/clicks.rs"]
mod clicks;
#[path = "window/forms.rs"]
mod forms;
#[path = "window/reading.rs"]
mod reading;
#[path = "window/selection.rs"]
mod selection;
#[path = "window/shell.rs"]
mod shell;
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};

/** An owned child and bounded receive wait prevent failed GUI checks from leaking engines. */
struct Window {
    child: Child,
    input: Option<ChildStdin>,
    output: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
    sequence: u64,
    pids: Vec<u32>,
}
impl Window {
    /** An ignored test requires an explicit operator command before opening native windows. */
    fn start() -> Self {
        Self::start_as(true)
    }
    /** A window the person controls: the agent is connected to observe but holds no tab. */
    fn start_human() -> Self {
        Self::start_as(false)
    }
    /** Every window launch is ephemeral, observable over the native protocol and allowed to capture. */
    fn start_as(agent_control: bool) -> Self {
        let mut args = vec!["desktop", "--test-profile", "--agent"];
        if agent_control {
            args.push("--agent-control");
        }
        args.extend([
            "--url",
            "demo",
            "--capture-dir",
            "/private/tmp/oya-native-window-tests",
        ]);
        let mut child = Command::new(env!("CARGO_BIN_EXE_mars"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    return;
                };
                if let Ok(value) = serde_json::from_str(&line)
                    && sender.send(value).is_err()
                {
                    return;
                }
            }
        });
        Self {
            child,
            input,
            output,
            reader: Some(reader),
            sequence: 0,
            pids: Vec::new(),
        }
    }
    /** Only versioned correlated messages reach the actual desktop runtime. */
    fn request(&mut self, op: &str, mut params: Value) -> Value {
        self.sequence += 1;
        let id = self.sequence.to_string();
        if op.starts_with("page.")
            || op == "control.release"
            || op == "window.resize"
            || op == "window.appearance"
        {
            params["tab_id"] = json!("tab-1");
        }
        let request = json!({"version":2,"id":id,"op":op,"params":params});
        self.input
            .as_mut()
            .unwrap()
            .write_all(&mars::protocol::encode(&request).unwrap())
            .unwrap();
        let response = self.output.recv_timeout(Duration::from_secs(12)).unwrap();
        assert_eq!(response["id"], id);
        response
    }
    /** Positive checks retain complete failures in their assertion output. */
    fn call(&mut self, op: &str, params: Value) -> Value {
        let response = self.request(op, params);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
    /** A capture must refer to the actual current painted revision and layout. */
    fn capture(&mut self) -> Value {
        let page = self.call("page.inspect", json!({}));
        let image=self.call("page.screenshot",json!({"tab_id":page["tab_id"],"page_id":page["page_id"],"expected_revision":page["revision"],"layout_generation":page["layout_generation"]}));
        let path = image["path"].as_str().unwrap();
        assert!(std::fs::metadata(path).unwrap().len() > 1024);
        println!("Native screenshot: {path}");
        image
    }
}
impl Drop for Window {
    /** Killing only this test owner releases pipes and all supervised children. */
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.input.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        support::assert_gone(&self.pids);
    }
}
/** Discovery finds controls semantically; mutation uses the opaque observed native target. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"control_epoch":1,"tab_id":page["tab_id"],"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}

/** Greeting assertions cover processed output and the unchanged native input in both viewport sizes. */
fn greet(window: &mut Window, name: &str, count: usize) {
    let page = window.call("page.inspect", json!({}));
    let mut params = target(&page, "name");
    params["text"] = json!(name);
    window.call("page.type", params);
    let page = window.call("page.inspect", json!({}));
    let changed = window.call("page.click", target(&page, "greet"));
    assert_greeting(&changed, name, count);
}

/** Visible text checks use the current document rather than a separately rendered fixture. */
fn assert_greeting(page: &Value, name: &str, count: usize) {
    let nodes = page["nodes"].as_array().unwrap();
    let greeting = if name.trim().is_empty() {
        "world"
    } else {
        name.trim()
    };
    assert!(nodes.iter().any(|node| node["html_id"] == "result"
        && node["text"] == format!("Hello, {greeting}! From your own Rust V9.")));
    assert!(nodes.iter().any(|node| node["html_id"] == "greeting-count"
        && node["text"] == format!("Greetings created: {count}")));
    assert!(
        nodes
            .iter()
            .any(|node| node["html_id"] == "name" && node["value"] == name)
    );
}

/** Actual OS captures verify the shared agent journey across normal/narrow/light/dark/error states. */
#[test]
#[ignore = "opens the actual native macOS window; run explicitly for desktop QA"]
fn visible_window_completes_agent_journey_and_captures_native_states() {
    let mut window = Window::start();
    let capabilities = window.call("runtime.capabilities", json!({}));
    window.pids = capabilities["engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|engine| engine["pid"].as_u64().unwrap() as u32)
        .collect();
    let initial = window.call("window.inspect", json!({}));
    assert_eq!(initial["title"], "Mars");
    assert!(initial["error"].is_null(), "{initial}");
    window.capture();
    let arithmetic = window.call("page.inspect", json!({}));
    assert!(
        arithmetic["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["html_id"] == "math" && node["text"] == "2 + 3 × 4 = 14")
    );
    let arithmetic = window.call("page.click", target(&arithmetic, "calculate"));
    assert!(
        arithmetic["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["html_id"] == "math" && node["text"] == "Saved result + 1 = 15")
    );
    window.capture();
    #[cfg(feature = "test-support")]
    {
        let before = window.call("page.inspect", json!({}));
        let url = format!(
            "{}/__test/v9-error",
            capabilities["demo_url"]
                .as_str()
                .unwrap()
                .trim_end_matches('/')
        );
        // A page whose script fails to parse still loads, as in a browser; the failed script is a
        // diagnostic on the page, not a navigation error, and Back returns to the demo.
        let broken = window.call("page.navigate", json!({"url":url,"control_epoch":1}));
        assert_ne!(broken["page_id"], before["page_id"]);
        assert!(
            broken["degraded"]
                .as_array()
                .unwrap()
                .iter()
                .any(|feature| feature == "scripts"),
            "{broken}"
        );
        assert!(
            broken["diagnostics"][0]
                .as_str()
                .unwrap()
                .contains("line 2, column 7"),
            "{broken}"
        );
        let state = window.call("window.inspect", json!({}));
        assert!(state["error"].is_null(), "{state}");
        assert_eq!(state["page"]["page_id"], broken["page_id"]);
        window.capture();
        let returned = window.call("page.back", json!({"control_epoch":1}));
        assert!(
            returned["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["html_id"] == "add"),
            "{returned}"
        );
    }
    for count in 1..=4 {
        let page = window.call("page.inspect", json!({}));
        let changed = window.call("page.click", target(&page, "add"));
        let nodes = changed["nodes"].as_array().unwrap();
        assert!(nodes.iter().any(|node| node["html_id"] == "count" && node["text"] == format!("Count: {count}")));
        assert!(nodes.iter().any(|node| node["html_id"] == "sum"
            && node["text"] == format!("Sum from 1 to count: {}", count * (count + 1) / 2)));
        assert!(nodes.iter().any(|node| node["html_id"] == "branch"
            && node["text"]
                == if count >= 3 {
                    "Target reached."
                } else {
                    "Keep going."
                }));
    }
    window.capture();
    let page = window.call("page.inspect", json!({}));
    let reset = window.call("page.click", target(&page, "reset"));
    assert!(
        reset["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["html_id"] == "sum" && node["text"] == "Sum from 1 to count: 0")
    );
    assert!(
        reset["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["html_id"] == "count" && node["text"] == "Count: 0")
    );
    window.capture();
    let page = window.call("page.inspect", json!({}));
    let geometry = window.call("page.layout", json!({}));
    let input = target(&page, "name");
    let y = geometry["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == input["node_id"])
        .unwrap()["y"]
        .as_f64()
        .unwrap();
    window.call("page.scroll",json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":(y-100.0).max(0.0),"control_epoch":1}));
    for (count, name) in [
        (1, "Ada"),
        (2, "Lin"),
        (3, "Lin"),
        (4, "Rust 🐙"),
        (5, ""),
        (6, "  Ada  "),
        (7, " \t "),
    ] {
        greet(&mut window, name, count);
        if count >= 6 {
            window.capture();
        }
    }
    window.capture();
    window.call(
        "window.appearance",
        json!({"appearance":"light","control_epoch":1}),
    );
    window.capture();
    window.call(
        "window.appearance",
        json!({"appearance":"dark","control_epoch":1}),
    );
    window.capture();
    window.call(
        "window.resize",
        json!({"width":640,"height":480,"control_epoch":1}),
    );
    for _ in 0..20 {
        let page = window.call("page.inspect", json!({}));
        if page["viewport"]["width"] == 640.0 {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let narrow = window.call("page.inspect", json!({}));
    assert_eq!(narrow["viewport"]["width"], 640.0);
    window.capture();
    for (count, name) in [(8, "  Ada  "), (9, " \t ")] {
        greet(&mut window, name, count);
        window.capture();
    }
    for id in ["next", "greet"] {
        let page = window.call("page.inspect", json!({}));
        let mut params = target(&page, id);
        params["text"] = json!(
            "A long native activation label must wrap exactly as the engine reports. ".repeat(6)
        );
        window.call("page.set_text", params);
    }
    let geometry = window.call("page.layout", json!({}));
    let page = window.call("page.inspect", json!({}));
    for id in ["next", "greet"] {
        let params = target(&page, id);
        let item = geometry["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["node_id"] == params["node_id"])
            .unwrap();
        assert!(item["lines"].as_array().unwrap().len() > 1);
    }
    window.capture();
    let before = narrow.clone();
    let error = window.request(
        "page.navigate",
        json!({"url":"http://169.254.169.254/","control_epoch":1}),
    );
    assert_eq!(error["error"]["code"], "permission_denied");
    assert_eq!(error["error"]["previous_page_retained"], true);
    thread::sleep(Duration::from_millis(100));
    let state = window.call("window.inspect", json!({}));
    assert!(
        state["error"]
            .as_str()
            .unwrap()
            .contains("Navigation blocked")
    );
    assert_eq!(state["page"]["page_id"], before["page_id"]);
    window.capture();
    let page = window.call("page.inspect", json!({}));
    let second = window.call("page.click", target(&page, "next"));
    assert!(second["url"].as_str().unwrap().ends_with("/second"));
    window.call("page.back", json!({"control_epoch":1}));
    window.call("page.forward", json!({"control_epoch":1}));
    window.call("page.reload", json!({"control_epoch":1}));
    window.call("control.release", json!({"control_epoch":1}));
    assert_eq!(window.call("control.status", json!({}))["owner"], "human");
    let denied = window.request("page.reload", json!({"control_epoch":1}));
    assert_eq!(denied["error"]["code"], "control_denied");
    window.input.take();
    thread::sleep(Duration::from_millis(100));
    assert!(window.child.try_wait().unwrap().is_none());
    assert!(window.pids.iter().all(|pid| support::process_exists(*pid)));
}

/** Native field-editor/delegate and target/action paths pin review fixes beyond agent requests. */
#[cfg(feature = "test-support")]
#[test]
#[ignore = "opens the actual native macOS window and exercises local human callbacks"]
fn native_human_input_and_failed_address_remain_authoritative() {
    let child = Command::new(env!("CARGO_BIN_EXE_mars"))
        .args([
            "desktop",
            "--verify-ui",
            "--url",
            "demo",
            "--capture-dir",
            "/private/tmp/oya-native-window-tests",
        ])
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
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Native human UI verification passed")
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Native human option selection verified")
    );
}
