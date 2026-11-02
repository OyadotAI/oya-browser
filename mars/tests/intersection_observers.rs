/*! Native layout, resize and scrolling deliver measured intersection changes through the owned V9 realm. */
#![cfg(target_os = "macos")]
#[allow(dead_code)]
mod support;
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner},
};
use serde_json::json;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use support::{Fixture, agent_call, html};

/** Bound native scheduling waits without accessing any public network endpoint. */
const DELIVERY_WALL: Duration = Duration::from_secs(3);
const DELIVERY_POLL: Duration = Duration::from_millis(25);
/** The narrowest window Eye lays out (its MIN_VIEWPORT is 280px), so a box past it starts hidden. */
const START_WIDTH: f64 = 300.0;

/** Observe a real positioned box and retain callback results in the committed document. */
fn page(position: &str) -> Vec<u8> {
    html(
        "",
        &format!(
            r#"<style>body{{margin:0;height:1200px}}#target{{position:absolute;{position};width:40px;height:40px}}</style>
        <p id=result>pending</p><div id=target>box</div><script>
        let observations=[];const observer=new IntersectionObserver(entries=>{{
            for(const entry of entries) observations.push(String(entry.isIntersecting));
            document.getElementById('result').textContent=observations.join('|');
        }});observer.observe(document.getElementById('target'));
        </script>"#
        ),
    )
}

/** Real services and native geometry run with explicit fixture-only network authority. */
fn load(fixture: &Fixture) -> Runtime {
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    runtime.set_viewport(START_WIDTH, 200.0).unwrap();
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    runtime
}

/** Snapshot reads cannot themselves execute or schedule observer callbacks. */
fn result(runtime: &Runtime) -> String {
    runtime
        .snapshot()
        .document
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|node| node.html_id == "result")
        .unwrap()
        .text
        .clone()
}

/** Only the ordinary background scheduler advances the fixture after native presentation changes. */
fn settle(runtime: &mut Runtime, expected: &str) {
    let deadline = Instant::now() + DELIVERY_WALL;
    while result(runtime) != expected && Instant::now() < deadline {
        std::thread::sleep(DELIVERY_POLL);
        runtime.run_page_scripts();
    }
    assert_eq!(result(runtime), expected);
}

/** Native resizing supplies fresh boxes even when the author scheduled no timers. */
#[test]
fn viewport_resize_delivers_a_new_intersection_without_author_timers() {
    let fixture = Fixture::raw(page("left:350px;top:20px"));
    let mut runtime = load(&fixture);
    settle(&mut runtime, "false");
    runtime.set_viewport(400.0, 200.0).unwrap();
    settle(&mut runtime, "false|true");
}

/** Document scrolling changes client intersections while preserving the target's layout position. */
#[test]
fn native_scrolling_delivers_entry_and_exit_changes() {
    let fixture = Fixture::raw(page("left:20px;top:600px"));
    let mut runtime = load(&fixture);
    settle(&mut runtime, "false");
    runtime.observe_scroll(500.0);
    settle(&mut runtime, "false|true");
    runtime.observe_scroll(0.0);
    settle(&mut runtime, "false|true|false");
}
