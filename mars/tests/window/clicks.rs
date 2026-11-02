/** Real mouse clicks on page content under the person's control: links, buttons, elements that
only a script listens to, links created by a script and links that came into reach by scrolling. */
use super::*;

/// Page state is polled this often while the engine and AppKit catch up.
const WAIT: Duration = Duration::from_millis(25);
/// Polling gives up after this many rounds (ten seconds), longer than any load or settle here.
const ATTEMPTS: usize = 400;
/// Far enough down the fixture that its last link, out of widget reach at load, comes into view.
const SCROLL_TO: f64 = 2000.0;
/// A click nothing listens to must stay silent; this is how long a banner would take to appear.
const SILENCE: Duration = Duration::from_millis(400);

/// The clicks fixture, served by the demo origin in test builds.
fn fixture_url(window: &mut Window) -> String {
    let capabilities = window.call("runtime.capabilities", json!({}));
    format!(
        "{}/__test/clicks",
        capabilities["demo_url"]
            .as_str()
            .unwrap()
            .trim_end_matches('/')
    )
}

/// Poll the window until the page on screen satisfies the condition, or fail with the last state.
fn wait(window: &mut Window, what: &str, condition: impl Fn(&Value) -> bool) -> Value {
    let mut state = Value::Null;
    for _ in 0..ATTEMPTS {
        state = window.call("window.inspect", json!({}));
        if condition(&state) {
            return state;
        }
        thread::sleep(WAIT);
    }
    panic!("window never reached: {what}\n{state}");
}

/// Whether the page on screen has finished loading this address, widgets included.
fn loaded(state: &Value, path: &str, widget: &str) -> bool {
    state["page"]["url"]
        .as_str()
        .is_some_and(|url| url.ends_with(path))
        && state["page"]["settling"].is_null()
        && state["error"].is_null()
        && has_frame(state, widget)
}

/// Whether a native widget exists for this element.
fn has_frame(state: &Value, html_id: &str) -> bool {
    state["native_frames"]
        .as_array()
        .is_some_and(|frames| frames.iter().any(|frame| frame["html_id"] == html_id))
}

/// The text of one element on the page on screen.
fn text(state: &Value, html_id: &str) -> Option<String> {
    state["page"]["nodes"]
        .as_array()?
        .iter()
        .find(|node| node["html_id"] == html_id)?["text"]
        .as_str()
        .map(str::to_owned)
}

/// Type the fixture address as the person and wait for it to be on screen.
fn open_fixture(window: &mut Window, url: &str) {
    window.call("window.human", json!({"action":"enter","text":url}));
    wait(window, "the clicks fixture", |state| {
        loaded(state, "/__test/clicks", "button")
    });
}

/// Press and release the mouse at the center of an element's display box.
fn click(window: &mut Window, html_id: &str) {
    let page = window.call("page.inspect", json!({}));
    let layout = window.call("page.layout", json!({}));
    let node_id = &page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == html_id)
        .unwrap_or_else(|| panic!("no {html_id} on the page"))["node_id"];
    let item = layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == *node_id)
        .unwrap();
    let x = item["x"].as_f64().unwrap() + item["width"].as_f64().unwrap() / 2.0;
    let y = item["y"].as_f64().unwrap() + item["height"].as_f64().unwrap() / 2.0;
    window.call("window.human", json!({"action":"click","point":[x, y]}));
}

/// Scroll the page as the person would and wait for the engine to hear it and create the widgets that came into reach.
fn scroll_to_far_link(window: &mut Window) {
    window.call(
        "window.human",
        json!({"action":"scroll","point":[0.0, SCROLL_TO]}),
    );
    wait(
        window,
        "widgets for the links that came into reach",
        |state| has_frame(state, "far") && has_frame(state, "late"),
    );
}

/// Every kind of clickable page content answers a real mouse click while the person holds control.
#[test]
#[ignore = "opens the actual native macOS window and clicks page content with real mouse events"]
fn native_clicks_reach_links_buttons_and_script_listeners_under_human_control() {
    let mut window = Window::start_human();
    let url = fixture_url(&mut window);
    let initial = window.call("window.inspect", json!({}));
    assert_eq!(initial["control"], "human", "{initial}");
    open_fixture(&mut window, &url);
    let state = window.call("window.inspect", json!({}));
    assert!(
        !has_frame(&state, "far"),
        "far link widget created before it was in reach"
    );

    click(&mut window, "button");
    wait(&mut window, "the button's listener to count", |state| {
        text(state, "count").as_deref() == Some("1")
    });
    click(&mut window, "div");
    wait(&mut window, "the div's listener to run", |state| {
        text(state, "log").as_deref() == Some("div")
    });
    click(&mut window, "role");
    wait(
        &mut window,
        "the role=button span's listener to run",
        |state| text(state, "log").as_deref() == Some("role"),
    );
    window.capture();

    click(&mut window, "title");
    thread::sleep(SILENCE);
    let quiet = window.call("window.inspect", json!({}));
    assert!(quiet["error"].is_null(), "{quiet}");
    assert_eq!(text(&quiet, "log").as_deref(), Some("role"));

    click(&mut window, "link");
    wait(&mut window, "the link to navigate", |state| {
        loaded(state, "/second", "home")
    });

    open_fixture(&mut window, &url);
    scroll_to_far_link(&mut window);
    click(&mut window, "far");
    wait(&mut window, "the far link to navigate", |state| {
        state["page"]["url"]
            .as_str()
            .is_some_and(|url| url.ends_with("/forms"))
    });

    open_fixture(&mut window, &url);
    scroll_to_far_link(&mut window);
    click(&mut window, "late");
    wait(
        &mut window,
        "the script-inserted link to navigate",
        |state| {
            state["page"]["url"]
                .as_str()
                .is_some_and(|url| url.ends_with("/compat/choose-option"))
        },
    );
}
