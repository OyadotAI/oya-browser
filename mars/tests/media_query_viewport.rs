/*! A window resize flips a media query: the page's MediaQueryList listener hears it while the page is idle, as a browser reports media query changes before its next task. */
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
/** A width above the query's break point, so the page starts with the query false. */
const WIDE: f64 = 1000.0;
/** A width under the query's break point (656px), so the resize flips it. */
const NARROW: f64 = 640.0;

/** A page whose listener writes what it saw: the query's result at the time of the event. */
fn page() -> Vec<u8> {
    html(
        "",
        r#"<p id=result>pending</p><script>
        const query=matchMedia('(max-width: 656px)');
        document.getElementById('result').textContent='before:'+query.matches;
        query.addEventListener('change',event=>{
            document.getElementById('result').textContent='after:'+event.matches+':'+query.matches;
        });
        </script>"#,
    )
}

/** Real services and native layout run with fixture-only network authority. */
fn load(fixture: &Fixture) -> Runtime {
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    runtime.set_viewport(WIDE, 200.0).unwrap();
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    runtime
}

/** The text of the page's result paragraph, read from the committed document. */
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

/** Only the background scheduler runs the page after the resize; no author timer is involved. */
fn settle(runtime: &mut Runtime, expected: &str) {
    let deadline = Instant::now() + DELIVERY_WALL;
    while result(runtime) != expected && Instant::now() < deadline {
        std::thread::sleep(DELIVERY_POLL);
        runtime.run_page_scripts();
    }
    assert_eq!(result(runtime), expected);
}

/** A resize that flips a query reaches the listener of a page with no timers of its own. */
#[test]
fn idle_resize_fires_the_media_query_change_listener() {
    let fixture = Fixture::raw(page());
    let mut runtime = load(&fixture);
    settle(&mut runtime, "before:false");
    runtime.set_viewport(NARROW, 200.0).unwrap();
    settle(&mut runtime, "after:true:true");
}
