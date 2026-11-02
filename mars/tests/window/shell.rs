/** Real mouse presses on the shell's own controls: the tab strip and the control pill sit in the
title bar, where a press that no control claims drags the window instead of clicking. */
use super::*;

/// Shell state is polled this often while AppKit and the coordinator catch up.
const WAIT: Duration = Duration::from_millis(25);
/// Polling gives up after this many rounds (ten seconds).
const ATTEMPTS: usize = 400;
/// The tab row's vertical centre in page points: above the page by the toolbar and half the strip.
const STRIP_Y: f64 = -66.0;
/// The first tab's centre: past the traffic lights and halfway into a full-width tab.
const FIRST_TAB_X: f64 = 200.0;
/// The new-tab button just after one full-width tab.
const NEW_TAB_X: f64 = 350.0;
/// The control pill's vertical centre in the toolbar.
const TOOLBAR_Y: f64 = -26.0;
/// The control pill sits this far in from the window's trailing edge.
const PILL_INSET: f64 = 70.0;
/// The first tab's select button tag (TAB_SELECT_BASE minus its index).
const FIRST_TAB_TAG: i64 = -1000;
/// The new-tab button's tag (TAB_NEW).
const NEW_TAB_TAG: i64 = -20;
/// The control pill's tag (CONTROL).
const CONTROL_TAG: i64 = -5;

/// Poll until the condition holds on the tab list, or fail with the last list.
fn wait_tabs(window: &mut Window, what: &str, condition: impl Fn(&Value) -> bool) -> Value {
    let mut tabs = Value::Null;
    for _ in 0..ATTEMPTS {
        tabs = window.call("tab.list", json!({}));
        if condition(&tabs) {
            return tabs;
        }
        thread::sleep(WAIT);
    }
    panic!("tabs never reached: {what}\n{tabs}");
}

/// Wait for the first page to finish so the shell is laid out and enabled.
fn ready(window: &mut Window) {
    for _ in 0..ATTEMPTS {
        let state = window.call("window.inspect", json!({}));
        if state["page"]["page_id"].is_string() && state["page"]["settling"].is_null() {
            return;
        }
        thread::sleep(WAIT);
    }
    panic!("the first page never finished");
}

/// Which control a press at this page point reaches.
fn hit(window: &mut Window, x: f64, y: f64) -> Value {
    window.call("window.human", json!({"action":"hit","point":[x, y]}))
}

/// Press and release the mouse at a page point through the application's event queue.
fn click(window: &mut Window, x: f64, y: f64) {
    window.call("window.human", json!({"action":"click","point":[x, y]}));
}

/// The owner of one tab in a tab list.
fn owner(tabs: &Value, tab_id: &str) -> String {
    tabs["tabs"]
        .as_array()
        .and_then(|tabs| tabs.iter().find(|tab| tab["tab_id"] == tab_id))
        .and_then(|tab| tab["owner"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// A press on a tab reaches that tab's button, never the window's title-bar drag.
#[test]
#[ignore = "opens a native window; run with --ignored"]
fn a_press_on_a_tab_reaches_its_button_not_the_window_drag() {
    let mut window = Window::start_human();
    ready(&mut window);
    let tab = hit(&mut window, FIRST_TAB_X, STRIP_Y);
    assert_eq!(tab["tag"], FIRST_TAB_TAG, "{tab}");
    assert_eq!(tab["moves_window"], false, "{tab}");
    let new_tab = hit(&mut window, NEW_TAB_X, STRIP_Y);
    assert_eq!(new_tab["tag"], NEW_TAB_TAG, "{new_tab}");
}

/// Clicking the new-tab button opens a tab, and clicking the first tab selects it again.
#[test]
#[ignore = "opens a native window; run with --ignored"]
fn clicking_the_strip_opens_and_selects_tabs() {
    let mut window = Window::start_human();
    ready(&mut window);
    click(&mut window, NEW_TAB_X, STRIP_Y);
    wait_tabs(&mut window, "a second tab, selected", |tabs| {
        tabs["tabs"].as_array().is_some_and(|tabs| tabs.len() == 2)
            && tabs["selected_tab_id"] != "tab-1"
    });
    click(&mut window, FIRST_TAB_X, STRIP_Y);
    wait_tabs(&mut window, "the first tab selected", |tabs| {
        tabs["selected_tab_id"] == "tab-1"
    });
}

/// While an agent holds the tab, the person can still click Take control, and the agent is then refused.
#[test]
#[ignore = "opens a native window; run with --ignored"]
fn take_control_works_while_the_agent_holds_the_tab() {
    let mut window = Window::start();
    ready(&mut window);
    let width = window.call("window.inspect", json!({}))["width"]
        .as_f64()
        .unwrap();
    let pill = hit(&mut window, width - PILL_INSET, TOOLBAR_Y);
    assert_eq!(pill["tag"], CONTROL_TAG, "{pill}");
    click(&mut window, width - PILL_INSET, TOOLBAR_Y);
    wait_tabs(&mut window, "the person owns the tab", |tabs| {
        owner(tabs, "tab-1") == "human"
    });
    let epoch = window.call("control.status", json!({}))["epoch"].clone();
    let refused = window.request(
        "page.navigate",
        json!({"url":"https://example.com/","control_epoch":epoch}),
    );
    assert_eq!(refused["error"]["code"], "control_denied", "{refused}");
}
