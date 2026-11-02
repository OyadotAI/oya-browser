/** A page that hides the viewport's overflow cannot be scrolled by a person's wheel or keys, as
Chromium holds a page still under a consent dialog; scripts and the agent's own scroll still move
it (CSS Overflow: `hidden` clips but stays programmatically scrollable). */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** The viewport offset after a wheel turn over the middle of `source`. */
fn wheeled(source: &str) -> (Value, Value) {
    let fixture = Fixture::routes(vec![("/", html("", source))]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let wheel = agent_call(
        &mut runtime,
        "page.scroll_at",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"x":100.0,"y":200.0,"delta_y":300.0}),
    )
    .unwrap();
    let script = agent_call(
        &mut runtime,
        "page.scroll",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":700.0}),
    )
    .unwrap();
    (
        wheel["viewport"]["scroll_y"].clone(),
        script["viewport"]["scroll_y"].clone(),
    )
}

/** `overflow: hidden` on the root stops the wheel; a programmatic scroll still moves. */
#[test]
fn root_overflow_hidden_stops_the_wheel() {
    let (wheel, script) = wheeled(
        "<!doctype html><title>Root</title><style>html{overflow:hidden}p{height:3000px}</style><p>tall</p>",
    );
    assert_eq!(wheel, 0.0);
    assert_eq!(script, 700.0);
}

/** Body's `overflow: hidden` propagates to the viewport while the root's is visible. */
#[test]
fn body_overflow_hidden_propagates_to_the_viewport() {
    let (wheel, _) = wheeled(
        "<!doctype html><title>Body</title><style>body{overflow:hidden}p{height:3000px}</style><p>tall</p>",
    );
    assert_eq!(wheel, 0.0);
}

/** Body's overflow stays on body once the root's is not visible, so the viewport scrolls. */
#[test]
fn body_overflow_does_not_propagate_past_a_scrolling_root() {
    let (wheel, _) = wheeled(
        "<!doctype html><title>Auto</title><style>html{overflow:auto}body{overflow:hidden}p{height:3000px}</style><p>tall</p>",
    );
    assert_eq!(wheel, 300.0);
}

/** A consent script that pins body (`position: fixed`, viewport high, overflow hidden) leaves
nothing below the fold to wheel to, as CNN's dialog does with `html{overflow-x:hidden}`. */
#[test]
#[ignore = "open: a fixed, viewport-high body still leaves the document extent at its content height"]
fn a_script_pinned_body_leaves_nothing_to_wheel() {
    let (wheel, _) = wheeled(
        "<!doctype html><title>Pinned</title><style>html{overflow-x:hidden}p{height:3000px}</style><p>tall</p>\
         <script>document.body.setAttribute('style','height:600px;width:800px;overflow:hidden;position:fixed')</script>",
    );
    assert_eq!(wheel, 0.0);
}
