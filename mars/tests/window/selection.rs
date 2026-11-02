/** Native agent selection verifies ordinary discovered controls, visible output and identity guards. */
use super::*;

/// Every lookup keeps the design's literal result and exercises the ordinary editable string.
const CASES: [(&str, &str, &str); 7] = [
    ("2", "Match found.", "Selected: Español (2, es)."),
    ("en", "Match found.", "Selected: English (1, en)."),
    ("ja", "Match found.", "Selected: 日本語 (3, ja)."),
    ("99", "No matching option.", "Try 1, 2, 3, en, es or ja."),
    (
        "",
        "Enter a number or language code.",
        "No option selected.",
    ),
    (
        " \t ",
        "Enter a number or language code.",
        "No option selected.",
    ),
    ("02", "Match found.", "Selected: Español (2, es)."),
];
/// Point comparisons tolerate rounding while exposing a displaced native activation surface.
const TOLERANCE: f64 = 0.01;
/// Resize is asynchronous and must converge before layout/capture assertions.
const RESIZE_ATTEMPTS: usize = 40;
/// A bounded short wait allows AppKit and the existing runtime queue to publish presentation.
const RESIZE_WAIT: Duration = Duration::from_millis(25);
/// The input should fit the narrow column and retain its authored maximum width.
const INPUT_MAXIMUM: f64 = 480.0;

/// Discovered ordinary controls exercise all lookup states in both native sizes and appearances.
#[test]
#[ignore = "opens the actual native macOS window for equality and array-selection QA"]
fn native_option_selection_preserves_controls_outputs_and_revision_guards() {
    let mut window = Window::start();
    let capabilities = window.call("runtime.capabilities", json!({}));
    window.pids = capabilities["engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|engine| engine["pid"].as_u64().unwrap() as u32)
        .collect();
    let demo = window.call("page.inspect", json!({}));
    assert_eq!(
        node(&demo, "choose-option")["text"],
        "Try choosing an option →"
    );
    let initial = window.call("page.click", target(&demo, "choose-option"));
    assert!(
        initial["url"]
            .as_str()
            .unwrap()
            .ends_with("/compat/choose-option")
    );
    assert_eq!(node(&initial, "option-query")["value"], "2");
    assert_outputs(&mut window, "Ready to check.", "No option selected.");
    for (profile, (width, height, appearance)) in [
        (1100, 760, "light"),
        (1100, 760, "dark"),
        (640, 480, "light"),
        (640, 480, "dark"),
    ]
    .into_iter()
    .enumerate()
    {
        presentation(&mut window, width, height, appearance);
        for (index, expected) in CASES.iter().enumerate() {
            let edit = if profile == 0 && index == 0 {
                None
            } else {
                Some(expected.0)
            };
            let page = choose(
                &mut window,
                edit,
                expected.1,
                expected.2,
                profile == 0 && index == 1,
            );
            assert_eq!(page["page_id"], initial["page_id"]);
            assert_eq!(
                node(&page, "option-query")["node_id"],
                node(&initial, "option-query")["node_id"]
            );
            assert_eq!(node(&page, "option-query")["value"], expected.0);
        }
        choose(
            &mut window,
            Some("ja"),
            "Match found.",
            "Selected: 日本語 (3, ja).",
            false,
        );
        assert_frames(&mut window);
        capture_outputs(&mut window);
    }
    let before = window.call("page.inspect", json!({}));
    let reloaded = window.call("page.reload", json!({"control_epoch":1}));
    assert_ne!(reloaded["page_id"], before["page_id"]);
    assert_eq!(node(&reloaded, "option-query")["value"], "2");
    assert_outputs(&mut window, "Ready to check.", "No option selected.");
    let stale = window.request("page.click", target(&before, "option-check"));
    assert_eq!(stale["error"]["code"], "invalid_target");
    let demo = window.call("page.click", target(&reloaded, "option-back"));
    assert!(
        demo["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["html_id"] == "greet")
    );
    window.call("control.release", json!({"control_epoch":1}));
    assert_eq!(window.call("control.status", json!({}))["owner"], "human");
    let denied = window.request("page.reload", json!({"control_epoch":1}));
    assert_eq!(denied["error"]["code"], "control_denied");
}

/// Editing alone retains both completed outputs; only the explicit button runs the callback.
fn choose(
    window: &mut Window,
    value: Option<&str>,
    status: &str,
    detail: &str,
    stale_check: bool,
) -> Value {
    let before = window.call("page.inspect", json!({}));
    if let Some(value) = value {
        let mut params = target(&before, "option-query");
        params["text"] = json!(value);
        window.call("page.type", params);
        let edited = window.call("page.inspect", json!({}));
        assert_eq!(
            node(&edited, "option-status")["text"],
            node(&before, "option-status")["text"]
        );
        assert_eq!(
            node(&edited, "option-result")["text"],
            node(&before, "option-result")["text"]
        );
        if stale_check {
            let stale = window.request("page.click", target(&before, "option-check"));
            assert_eq!(stale["error"]["code"], "revision_conflict");
            let retained = window.call("page.inspect", json!({}));
            assert_eq!(retained["revision"], edited["revision"]);
            assert_eq!(
                node(&retained, "option-status")["text"],
                node(&before, "option-status")["text"]
            );
        }
    }
    let current = window.call("page.inspect", json!({}));
    window.call("page.click", target(&current, "option-check"));
    assert_outputs(window, status, detail)
}

/// Native snapshots, immutable display text and agent inspection describe the same completed lookup.
fn assert_outputs(window: &mut Window, status: &str, detail: &str) -> Value {
    let page = window.call("page.inspect", json!({}));
    let layout = window.call("page.layout", json!({}));
    let state = window.call("window.inspect", json!({}));
    assert_eq!(state["control"], "agent");
    assert!(state["error"].is_null(), "{state}");
    assert_eq!(state["page"]["page_id"], page["page_id"]);
    assert_eq!(state["page"]["revision"], page["revision"]);
    for (id, expected) in [("option-status", status), ("option-result", detail)] {
        assert_eq!(node(&page, id)["text"], expected);
        let painted = item(&layout, &page, id)["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line["text"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(painted, expected);
    }
    page
}

/// Actual widgets use current engine boxes rather than a second geometry chosen by AppKit.
fn assert_frames(window: &mut Window) {
    let page = window.call("page.inspect", json!({}));
    let layout = window.call("page.layout", json!({}));
    let state = window.call("window.inspect", json!({}));
    for id in ["option-query", "option-check", "option-back"] {
        let item = item(&layout, &page, id);
        let frame = state["native_frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["html_id"] == id)
            .unwrap();
        for key in ["x", "y", "width"] {
            close(number(frame, key), number(item, key));
        }
        close(
            number(frame, "height"),
            item["control_height"]
                .as_f64()
                .unwrap_or_else(|| number(item, "height")),
        );
    }
    let input = item(&layout, &page, "option-query");
    assert!(number(input, "width") <= INPUT_MAXIMUM + TOLERANCE);
    assert!(number(input, "x") + number(input, "width") <= number(&page["viewport"], "width"));
}

/// Viewport convergence precedes native theme acknowledgment and avoids taking stale resize captures.
fn presentation(window: &mut Window, width: usize, height: usize, appearance: &str) {
    window.call(
        "window.resize",
        json!({"width":width,"height":height,"control_epoch":1}),
    );
    let mut converged = false;
    for _ in 0..RESIZE_ATTEMPTS {
        let state = window.call("window.inspect", json!({}));
        let native = &state["native_banner"]["page_content"];
        let context = &state["render_context"];
        if number(&state, "width") == width as f64
            && number(&state, "height") == height as f64
            && (number(native, "width") - number(context, "width")).abs() <= TOLERANCE
            && (number(native, "height") - number(context, "height")).abs() <= TOLERANCE
        {
            converged = true;
            break;
        }
        thread::sleep(RESIZE_WAIT);
    }
    assert!(converged, "selection viewport did not converge");
    window.call(
        "window.appearance",
        json!({"appearance":appearance,"control_epoch":1}),
    );
    let state = window.call("window.inspect", json!({}));
    assert_eq!(state["render_context"]["appearance"], appearance);
    assert_frames(window);
}

/// Narrow screenshots scroll to the real output region while retaining the native input and action nearby.
fn capture_outputs(window: &mut Window) {
    let page = window.call("page.inspect", json!({}));
    let layout = window.call("page.layout", json!({}));
    let y = (number(item(&layout, &page, "option-status"), "y")
        - number(&page["viewport"], "height") / 2.0)
        .max(0.0);
    window.call("page.scroll", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":y,"control_epoch":1}));
    window.capture();
}

/// Semantic lookup names fixed fixture controls while mutations retain discovered opaque handles.
fn node<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()
}

/// Display lookup matches the exact inspection node identity used by guarded native actions.
fn item<'a>(layout: &'a Value, page: &Value, id: &str) -> &'a Value {
    layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == node(page, id)["node_id"])
        .unwrap()
}

/// Missing geometry reports its actual observation instead of fabricating a default coordinate.
fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("missing native {key} in {value}"))
}

/// A single point tolerance gives useful actual-versus-expected frame failures.
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "actual {actual}, expected {expected}"
    );
}
